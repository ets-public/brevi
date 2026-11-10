"""DeepSeek for the e2e benchmark, through its OpenAI-compatible Chat Completions API (api.deepseek.com).

`Client` looks like the part of `anthropic.Anthropic` that e2e.solve uses: `client.messages.stream(...)` as a
context manager whose `get_final_message()` returns a message with Anthropic-shaped `content` blocks, `usage`
and `stop_reason`. So e2e.py and replay.py run DeepSeek with the same loop, tools, check feedback and logs as
Claude; only this file knows the wire format. Standard library only (no `openai` package needed).

Mapping:
  system prompt blocks         -> one system message (DeepSeek caches repeated prefixes on its own; cache_control is dropped)
  user text / tool_result      -> user message / one `tool` message per result
  assistant thinking/text/tool_use -> assistant message with reasoning_content, content and tool_calls
  usage: prompt_cache_miss_tokens -> input_tokens, prompt_cache_hit_tokens -> cache_read_input_tokens,
         completion_tokens (thinking included) -> output_tokens; DeepSeek bills no cache writes.
  finish_reason tool_calls/stop/length -> stop_reason tool_use/end_turn/max_tokens

In thinking mode DeepSeek wants the reasoning_content of every assistant message in the current tool-calling
turn sent back, and a whole e2e task is one turn, so each assistant message carries its reasoning back.

Needs DEEPSEEK_API_KEY. DEEPSEEK_BASE_URL overrides https://api.deepseek.com (the unit test points it at a fake).
"""
import json, os, time, urllib.error, urllib.request

BASE_URL = "https://api.deepseek.com"


class Block:
    """An Anthropic-style content block: thinking, text or tool_use."""

    def __init__(self, **kw):
        self.__dict__.update(kw)

    def model_dump(self, exclude_none=True):
        return {k: v for k, v in self.__dict__.items() if v is not None or not exclude_none}


class Usage:
    def __init__(self, u):
        hit = u.get("prompt_cache_hit_tokens", (u.get("prompt_tokens_details") or {}).get("cached_tokens", 0)) or 0
        miss = u.get("prompt_cache_miss_tokens")
        self.input_tokens = miss if miss is not None else max(0, u.get("prompt_tokens", 0) - hit)
        self.cache_read_input_tokens = hit
        self.cache_creation_input_tokens = 0
        self.output_tokens = u.get("completion_tokens", 0)
        self.reasoning_tokens = (u.get("completion_tokens_details") or {}).get("reasoning_tokens")


class Message:
    def __init__(self, content, usage, stop_reason):
        self.content, self.usage, self.stop_reason = content, usage, stop_reason


STOP = {"tool_calls": "tool_use", "stop": "end_turn", "length": "max_tokens"}


def _get(b, k, default=None):
    return b.get(k, default) if isinstance(b, dict) else getattr(b, k, default)


def to_openai(system, messages):
    """Anthropic-shaped system + messages -> OpenAI chat messages."""
    sys_text = system if isinstance(system, str) else "\n\n".join(_get(b, "text", "") for b in system)
    out = [{"role": "system", "content": sys_text}]
    for m in messages:
        c = m["content"]
        if isinstance(c, str):
            out.append({"role": m["role"], "content": c})
            continue
        if m["role"] == "assistant":
            text = "".join(_get(b, "text", "") for b in c if _get(b, "type") == "text")
            reasoning = "".join(_get(b, "thinking", "") for b in c if _get(b, "type") == "thinking")
            calls = [{"id": _get(b, "id"), "type": "function",
                      "function": {"name": _get(b, "name"), "arguments": json.dumps(_get(b, "input") or {})}}
                     for b in c if _get(b, "type") == "tool_use"]
            msg = {"role": "assistant", "content": text}
            if reasoning:
                msg["reasoning_content"] = reasoning
            if calls:
                msg["tool_calls"] = calls
            out.append(msg)
            continue
        texts = []
        for b in c:
            t = _get(b, "type")
            if t == "tool_result":
                body = _get(b, "content")
                if not isinstance(body, str):
                    body = "".join(_get(x, "text", "") for x in body)
                if _get(b, "is_error"):
                    body = "error: " + body if body else "error"
                out.append({"role": "tool", "tool_call_id": _get(b, "tool_use_id"), "content": body})
            elif t == "text":
                texts.append(_get(b, "text"))
        if texts:
            out.append({"role": "user", "content": "\n\n".join(texts)})
    return out


def to_openai_tools(tools):
    # strict is a beta feature on DeepSeek (beta base URL only), so it is left out; the schemas are tiny anyway.
    return [{"type": "function", "function": {"name": t["name"], "description": t["description"], "parameters": t["input_schema"]}} for t in tools]


def from_openai(resp):
    ch = resp["choices"][0]
    m = ch["message"]
    blocks = []
    if m.get("reasoning_content"):
        blocks.append(Block(type="thinking", thinking=m["reasoning_content"]))
    if m.get("content"):
        blocks.append(Block(type="text", text=m["content"]))
    for tc in m.get("tool_calls") or []:
        try:
            args = json.loads(tc["function"].get("arguments") or "{}")
        except ValueError:
            # a malformed call still goes back as a tool call, so the model sees the error and can retry
            args = {"_unparsed": tc["function"].get("arguments")}
        blocks.append(Block(type="tool_use", id=tc["id"], name=tc["function"]["name"], input=args))
    stop = STOP.get(ch.get("finish_reason"), ch.get("finish_reason"))
    if stop == "end_turn" and any(b.type == "tool_use" for b in blocks):
        stop = "tool_use"
    return Message(blocks, Usage(resp.get("usage") or {}), stop)


class _Stream:
    def __init__(self, client, kw):
        self.client, self.kw = client, kw

    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False

    def get_final_message(self):
        return self.client._create(**self.kw)


class _Messages:
    def __init__(self, client):
        self.client = client

    def stream(self, **kw):
        return _Stream(self.client, kw)

    def create(self, **kw):
        return self.client._create(**kw)


class Client:
    def __init__(self, api_key=None, base_url=None, timeout=900, max_retries=4):
        self.api_key = api_key or os.environ.get("DEEPSEEK_API_KEY")
        if not self.api_key:
            raise RuntimeError("DEEPSEEK_API_KEY is not set")
        self.base_url = (base_url or os.environ.get("DEEPSEEK_BASE_URL") or BASE_URL).rstrip("/")
        self.timeout, self.max_retries = timeout, max_retries
        self.messages = _Messages(self)

    def request(self, model, system, messages, tools=(), max_tokens=32000, effort=None, thinking="adaptive"):
        body = {"model": model, "messages": to_openai(system, messages), "max_tokens": max_tokens, "stream": False}
        if tools:
            body["tools"] = to_openai_tools(tools)
        if thinking == "adaptive":
            body["thinking"] = {"type": "enabled"}
            if effort:
                # DeepSeek takes none/low/high/max and maps medium and xhigh to high
                body["reasoning_effort"] = effort
        else:
            body["thinking"] = {"type": "disabled"}
        return body

    def _create(self, model, system, messages, tools=(), max_tokens=32000, output_config=None, thinking=None, **_ignored):
        mode = "adaptive" if not thinking or thinking.get("type") in ("adaptive", "enabled") else "disabled"
        body = self.request(model, system, messages, tools, max_tokens, (output_config or {}).get("effort"), mode)
        return from_openai(self._post("/chat/completions", body))

    def _post(self, path, body):
        data = json.dumps(body).encode()
        for attempt in range(self.max_retries + 1):
            req = urllib.request.Request(self.base_url + path, data, {
                "Authorization": f"Bearer {self.api_key}", "Content-Type": "application/json", "Accept": "application/json"})
            try:
                with urllib.request.urlopen(req, timeout=self.timeout) as r:
                    # non-streamed answers may start with keep-alive blank lines while the model thinks
                    return json.loads(r.read().decode())
            except urllib.error.HTTPError as e:
                detail = e.read().decode(errors="replace")[:500]
                # 429 rate limit, 500/503 overload: retry; 400/401/402/422 are our fault or the account's
                if e.code in (429, 500, 502, 503, 504) and attempt < self.max_retries:
                    time.sleep(2 ** attempt * 2)
                    continue
                raise RuntimeError(f"DeepSeek API {e.code}: {detail}") from None
            except (urllib.error.URLError, TimeoutError, ConnectionError):
                if attempt < self.max_retries:
                    time.sleep(2 ** attempt * 2)
                    continue
                raise
