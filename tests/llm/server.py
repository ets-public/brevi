# A stand-in for the Anthropic and OpenAI APIs that the tests/llm programs talk to: `python3 tests/llm/server.py
# PORT` serves 127.0.0.1:PORT. Every answer is worked out from the request alone (ids and request-ids are hashes
# of the body), so a program's output can be compared with the .expected file the real SDKs printed.
#
# Anthropic: POST /v1/messages (also stream), /v1/messages/count_tokens, GET /v1/models, /v1/models/{id}.
# OpenAI: POST /v1/chat/completions (also stream), /v1/embeddings, /v1/responses (also stream), GET /v1/models.
# The model name picks a behaviour: "mock-error-CODE" answers that status, "mock-flaky" fails its first try (529
# / 500) and answers the retry, "mock-stream-error" breaks a stream with an error event.
import base64
import hashlib
import http.server
import json
import struct
import sys
import threading
from urllib.parse import parse_qs, urlsplit

KEY = "test-key"
ERRTYPE = {400: "invalid_request_error", 401: "authentication_error", 403: "permission_error", 404: "not_found_error",
           409: "conflict_error", 413: "request_too_large", 422: "invalid_request_error", 429: "rate_limit_error",
           500: "api_error", 503: "api_error", 529: "overloaded_error"}
MODELS = [
    ("claude-mock-opus", "Claude Mock Opus", "2026-05-01T00:00:00Z"),
    ("claude-mock-sonnet", "Claude Mock Sonnet", "2026-04-02T10:20:30Z"),
    ("claude-mock-haiku", "Claude Mock Haiku", "2026-01-15T00:00:00Z"),
]


def h(b, n=12):
    return hashlib.sha1(b).hexdigest()[:n]


def text_of(content):
    if isinstance(content, str):
        return content
    out = []
    for b in content or []:
        if isinstance(b, dict) and b.get("type") in ("text", "input_text", "output_text"):
            out.append(b.get("text", ""))
    return " ".join(out)


def tool_input(schema, said):
    out = {}
    for name, p in (schema or {}).get("properties", {}).items():
        t = p.get("type")
        if t == "string":
            out[name] = p["enum"][0] if "enum" in p else said[:40]
        elif t == "integer":
            out[name] = 42
        elif t == "number":
            out[name] = 2.5
        elif t == "boolean":
            out[name] = True
        elif t == "array":
            out[name] = [said[:10]]
        else:
            out[name] = {"note": said[:10]}
    return out


class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    wbufsize = 1 << 16

    def log_message(self, *a):
        pass

    def send(self, status, body, ctype="application/json", headers=()):
        self.send_response_only(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        for k, v in headers:
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def js(self, obj, status=200, headers=()):
        self.send(status, json.dumps(obj).encode(), headers=[("request-id", "req_" + self.rid)] + list(headers))

    def sse(self, events):
        self.send_response_only(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Transfer-Encoding", "chunked")
        self.send_header("request-id", "req_" + self.rid)
        self.send_header("x-request-id", "req_" + self.rid)
        self.end_headers()
        for ev in events:
            b = ev.encode()
            self.wfile.write(b"%x\r\n%s\r\n" % (len(b), b))
            self.wfile.flush()
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()

    def read_body(self):
        n = int(self.headers.get("Content-Length") or 0)
        return self.rfile.read(n) if n else b""

    def do_GET(self):
        self.route("GET", b"")

    def do_POST(self):
        self.route("POST", self.read_body())

    def do_DELETE(self):
        self.route("DELETE", self.read_body())

    def route(self, method, raw):
        u = urlsplit(self.path)
        q = {k: v[-1] for k, v in parse_qs(u.query).items()}
        self.rid = h(raw + u.path.encode() + u.query.encode())
        openai = u.path.startswith("/openai/")
        path = u.path[len("/openai"):] if openai else u.path
        auth = self.headers.get("Authorization", "")
        if auth != "Bearer " + KEY and self.headers.get("x-api-key") != KEY:
            return self.error(401, "invalid x-api-key" if not openai else "Incorrect API key provided: " + auth[7:], openai)
        body = json.loads(raw) if raw else {}
        model = body.get("model", "") if isinstance(body, dict) else ""
        if model.startswith("mock-error-"):
            code = int(model.split("-")[2])
            extra = [("retry-after-ms", "10")] if code == 429 else []
            return self.error(code, "mock error %d for %s" % (code, path), openai, extra)
        if model == "mock-flaky" and self.headers.get("x-stainless-retry-count", "0") == "0":
            return self.error(529 if not openai else 500, "Overloaded" if not openai else "The server had an error", openai, [("retry-after-ms", "20")])
        if path.startswith("/echo-headers"):
            names = ["content-type", "accept", "anthropic-version", "x-stainless-lang", "x-stainless-package-version",
                     "x-stainless-retry-count", "x-stainless-timeout", "x-stainless-read-timeout", "x-stainless-async",
                     "x-custom", "authorization", "x-api-key", "openai-organization", "openai-project"]
            got = {k: self.headers.get(k) for k in names if self.headers.get(k) is not None}
            return self.js({"headers": got, "query": u.query, "body": raw.decode()})
        if not openai:
            if path == "/v1/messages" and method == "POST":
                return self.messages(body, raw)
            if path == "/v1/messages/count_tokens":
                return self.js({"input_tokens": 3 + len(json.dumps(body.get("messages"))) // 4})
            if path == "/v1/models":
                return self.anthropic_models(q)
            if path.startswith("/v1/models/"):
                mid = path.split("/")[3]
                for m in MODELS:
                    if m[0] == mid:
                        return self.js(self.model_info(m))
                return self.error(404, "model: " + mid, False)
        else:
            if path == "/v1/chat/completions":
                return self.chat(body, raw)
            if path == "/v1/embeddings":
                return self.embeddings(body)
            if path == "/v1/responses":
                return self.resp_api(body, raw)
            if path == "/v1/models":
                return self.js({"object": "list", "data": [{"id": m[0].replace("claude", "gpt"), "object": "model", "created": 1767225600 + k, "owned_by": "mock"} for k, m in enumerate(MODELS)]})
        return self.error(404, "Not found: " + path, openai)

    def error(self, code, msg, openai, headers=()):
        if openai:
            obj = {"error": {"message": msg, "type": "invalid_request_error" if code < 500 else "server_error", "param": None, "code": "mock_%d" % code}}
        else:
            obj = {"type": "error", "error": {"type": ERRTYPE.get(code, "api_error"), "message": msg}, "request_id": "req_" + self.rid}
        self.js(obj, code, [("x-should-retry", "false")] if code == 400 else headers)

    # ---------- Anthropic ----------

    def model_info(self, m):
        return {"type": "model", "id": m[0], "display_name": m[1], "created_at": m[2], "max_input_tokens": 200000, "max_tokens": 64000}

    def anthropic_models(self, q):
        lim = int(q.get("limit", 20))
        start = 0
        if "after_id" in q:
            start = [m[0] for m in MODELS].index(q["after_id"]) + 1
        page = MODELS[start:start + lim]
        self.js({"data": [self.model_info(m) for m in page], "has_more": start + lim < len(MODELS),
                 "first_id": page[0][0] if page else None, "last_id": page[-1][0] if page else None})

    def answer(self, body):
        # (content blocks, stop_reason, stop_sequence)
        msgs = body.get("messages", [])
        last = msgs[-1] if msgs else {"content": ""}
        said = text_of(last.get("content"))
        results = [b for b in (last.get("content") if isinstance(last.get("content"), list) else []) if isinstance(b, dict) and b.get("type") == "tool_result"]
        blocks = []
        thinking = body.get("thinking") or {}
        if thinking.get("type") in ("enabled", "adaptive"):
            blocks.append({"type": "thinking", "thinking": "The user wrote %d characters." % len(said), "signature": "sig_" + h(said.encode(), 8)})
        tools = body.get("tools") or []
        choice = (body.get("tool_choice") or {}).get("type", "auto")
        if results:
            outs = []
            for r in results:
                c = r.get("content")
                outs.append(c if isinstance(c, str) else text_of(c))
            blocks.append({"type": "text", "text": "The tool said: " + "; ".join(outs)})
            return blocks, "end_turn", None
        if tools and choice != "none":
            t = tools[0]
            if choice == "tool":
                t = [x for x in tools if x.get("name") == body["tool_choice"]["name"]][0]
            blocks.append({"type": "text", "text": "I will use %s." % t["name"]})
            blocks.append({"type": "tool_use", "id": "toolu_" + h(json.dumps(msgs).encode(), 16), "name": t["name"], "input": tool_input(t.get("input_schema"), said)})
            return blocks, "tool_use", None
        system = body.get("system")
        text = "Echo: " + said
        if system:
            text += " [system: %s]" % (system if isinstance(system, str) else text_of(system))
        if len(msgs) > 1:
            text += " (turn %d)" % (len(msgs) // 2 + 1)
        stop, seq = "end_turn", None
        for s in body.get("stop_sequences") or []:
            if s in text:
                text, stop, seq = text[:text.index(s)], "stop_sequence", s
                break
        words = text.split(" ")
        if len(words) > body.get("max_tokens", 1 << 30):
            text, stop = " ".join(words[:body["max_tokens"]]), "max_tokens"
        blocks.append({"type": "text", "text": text})
        return blocks, stop, seq

    def messages(self, body, raw):
        blocks, stop, seq = self.answer(body)
        mid = "msg_" + h(raw, 20)
        usage = {"input_tokens": 5 + len(json.dumps(body.get("messages"))) // 4, "output_tokens": sum(len(json.dumps(b)) // 6 for b in blocks),
                 "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0, "service_tier": "standard"}
        if not body.get("stream"):
            return self.js({"id": mid, "type": "message", "role": "assistant", "model": body.get("model"), "content": blocks,
                            "stop_reason": stop, "stop_sequence": seq, "usage": usage})
        ev = []

        def e(name, obj):
            ev.append("event: %s\ndata: %s\n\n" % (name, json.dumps(obj)))

        e("message_start", {"type": "message_start", "message": {"id": mid, "type": "message", "role": "assistant", "model": body.get("model"), "content": [],
                                                                 "stop_reason": None, "stop_sequence": None, "usage": dict(usage, output_tokens=1)}})
        for i, b in enumerate(blocks):
            if b["type"] == "text":
                e("content_block_start", {"type": "content_block_start", "index": i, "content_block": {"type": "text", "text": ""}})
                words = b["text"].split(" ")
                for k in range(0, len(words), 2):
                    piece = " ".join(words[k:k + 2]) + (" " if k + 2 < len(words) else "")
                    e("content_block_delta", {"type": "content_block_delta", "index": i, "delta": {"type": "text_delta", "text": piece}})
                    if k == 0 and i == 0:
                        ev.append("event: ping\ndata: {\"type\": \"ping\"}\n\n")
                    if body.get("model") == "mock-stream-error":
                        e("error", {"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}})
                        return self.sse(ev)
            elif b["type"] == "thinking":
                e("content_block_start", {"type": "content_block_start", "index": i, "content_block": {"type": "thinking", "thinking": "", "signature": ""}})
                e("content_block_delta", {"type": "content_block_delta", "index": i, "delta": {"type": "thinking_delta", "thinking": b["thinking"]}})
                e("content_block_delta", {"type": "content_block_delta", "index": i, "delta": {"type": "signature_delta", "signature": b["signature"]}})
            else:
                e("content_block_start", {"type": "content_block_start", "index": i, "content_block": dict(b, input={})})
                js = json.dumps(b["input"])
                for k in range(0, len(js), 9):
                    e("content_block_delta", {"type": "content_block_delta", "index": i, "delta": {"type": "input_json_delta", "partial_json": js[k:k + 9]}})
            e("content_block_stop", {"type": "content_block_stop", "index": i})
        e("message_delta", {"type": "message_delta", "delta": {"stop_reason": stop, "stop_sequence": seq}, "usage": {"output_tokens": usage["output_tokens"]}})
        e("message_stop", {"type": "message_stop"})
        self.sse(ev)

    # ---------- OpenAI ----------

    def chat_answer(self, body):
        msgs = body.get("messages", [])
        last = msgs[-1] if msgs else {"content": ""}
        tools = body.get("tools") or []
        choice = body.get("tool_choice", "auto")
        if last.get("role") == "tool":
            outs = [m.get("content") for m in msgs if m.get("role") == "tool"]
            return {"role": "assistant", "content": "The tool said: " + "; ".join(text_of(o) for o in outs)}, "stop"
        said = text_of(last.get("content"))
        if tools and choice != "none":
            calls = []
            for k, t in enumerate(tools[:2] if body.get("parallel_tool_calls", True) else tools[:1]):
                f = t["function"]
                calls.append({"id": "call_" + h(json.dumps([msgs, k]).encode(), 16), "type": "function",
                              "function": {"name": f["name"], "arguments": json.dumps(tool_input(f.get("parameters"), said))}})
            return {"role": "assistant", "content": None, "tool_calls": calls}, "tool_calls"
        system = [text_of(m.get("content")) for m in msgs if m.get("role") in ("system", "developer")]
        text = "Echo: " + said + ((" [system: %s]" % system[0]) if system else "")
        stop = body.get("stop")
        for s in ([stop] if isinstance(stop, str) else stop or []):
            if s in text:
                text = text[:text.index(s)]
        reason = "stop"
        lim = body.get("max_completion_tokens") or body.get("max_tokens")
        if lim and len(text.split(" ")) > lim:
            text, reason = " ".join(text.split(" ")[:lim]), "length"
        return {"role": "assistant", "content": text}, reason

    def chat(self, body, raw):
        msg, reason = self.chat_answer(body)
        cid = "chatcmpl-" + h(raw, 20)
        pt = 7 + len(json.dumps(body.get("messages"))) // 4
        ct = len(json.dumps(msg)) // 6
        usage = {"prompt_tokens": pt, "completion_tokens": ct, "total_tokens": pt + ct,
                 "prompt_tokens_details": {"cached_tokens": 0, "audio_tokens": 0},
                 "completion_tokens_details": {"reasoning_tokens": 0, "audio_tokens": 0, "accepted_prediction_tokens": 0, "rejected_prediction_tokens": 0}}
        created = 1767225600 + int(h(raw, 4), 16)
        if not body.get("stream"):
            m = dict(msg, refusal=None, annotations=[])
            choices = [{"index": i, "message": m, "logprobs": None, "finish_reason": reason} for i in range(body.get("n", 1))]
            return self.js({"id": cid, "object": "chat.completion", "created": created, "model": body.get("model"), "choices": choices,
                            "usage": usage, "service_tier": "default", "system_fingerprint": "fp_mock"})
        ev = []

        def chunk(delta, finish=None, choices=True, use=None):
            obj = {"id": cid, "object": "chat.completion.chunk", "created": created, "model": body.get("model"), "service_tier": "default",
                   "system_fingerprint": "fp_mock", "choices": [{"index": 0, "delta": delta, "logprobs": None, "finish_reason": finish}] if choices else []}
            if (body.get("stream_options") or {}).get("include_usage"):
                obj["usage"] = use
            ev.append("data: %s\n\n" % json.dumps(obj))

        chunk({"role": "assistant", "content": "" if msg.get("content") is not None else None, "refusal": None})
        if msg.get("content"):
            words = msg["content"].split(" ")
            for k in range(0, len(words), 2):
                chunk({"content": " ".join(words[k:k + 2]) + (" " if k + 2 < len(words) else "")})
                if body.get("model") == "mock-stream-error":
                    ev.append('data: {"error": {"message": "stream broke", "type": "server_error", "param": null, "code": null}}\n\n')
                    return self.sse(ev)
        for i, c in enumerate(msg.get("tool_calls") or []):
            chunk({"tool_calls": [{"index": i, "id": c["id"], "type": "function", "function": {"name": c["function"]["name"], "arguments": ""}}]})
            a = c["function"]["arguments"]
            for k in range(0, len(a), 8):
                chunk({"tool_calls": [{"index": i, "function": {"arguments": a[k:k + 8]}}]})
        chunk({}, reason)
        if (body.get("stream_options") or {}).get("include_usage"):
            chunk(None, choices=False, use=usage)
        ev.append("data: [DONE]\n\n")
        self.sse(ev)

    def embeddings(self, body):
        inp = body.get("input")
        items = [inp] if isinstance(inp, str) else inp
        dims = body.get("dimensions", 6)
        data = []
        for i, t in enumerate(items):
            d = hashlib.sha256(str(t).encode()).digest()
            vec = [round((d[k % 32] - 128) / 128.0, 6) for k in range(dims)]
            if body.get("encoding_format") == "base64":
                emb = base64.b64encode(struct.pack("<%df" % dims, *vec)).decode()
            else:
                emb = vec
            data.append({"object": "embedding", "index": i, "embedding": emb})
        n = sum(len(str(t).split()) for t in items)
        self.js({"object": "list", "data": data, "model": body.get("model"), "usage": {"prompt_tokens": n, "total_tokens": n}})

    def resp_api(self, body, raw):
        inp = body.get("input")
        if isinstance(inp, str):
            said = inp
            outs = []
        else:
            said = text_of((inp or [{}])[-1].get("content")) if inp else ""
            outs = [x.get("output") for x in inp or [] if isinstance(x, dict) and x.get("type") == "function_call_output"]
        rid = "resp_" + h(raw, 24)
        tools = [t for t in body.get("tools") or [] if t.get("type") == "function"]
        output = []
        if outs:
            text = "The tool said: " + "; ".join(outs)
        elif tools and body.get("tool_choice", "auto") != "none":
            t = tools[0]
            output.append({"type": "function_call", "id": "fc_" + h(raw, 16), "call_id": "call_" + h(raw + b"c", 16), "name": t["name"],
                           "arguments": json.dumps(tool_input(t.get("parameters"), said)), "status": "completed"})
            text = None
        else:
            text = "Echo: " + said + ((" [instructions: %s]" % body["instructions"]) if body.get("instructions") else "")
        if text is not None:
            output.append({"type": "message", "id": "msg_" + h(raw, 16), "status": "completed", "role": "assistant",
                           "content": [{"type": "output_text", "text": text, "annotations": [], "logprobs": []}]})
        it = 6 + len(json.dumps(inp)) // 4
        ot = len(json.dumps(output)) // 6
        resp = {"id": rid, "object": "response", "created_at": 1767225600 + int(h(raw, 4), 16), "status": "completed", "background": False,
                "error": None, "incomplete_details": None, "instructions": body.get("instructions"), "max_output_tokens": body.get("max_output_tokens"),
                "model": body.get("model"), "output": output, "parallel_tool_calls": True, "previous_response_id": body.get("previous_response_id"),
                "reasoning": {"effort": None, "summary": None}, "service_tier": "default", "store": True, "temperature": 1.0,
                "text": {"format": {"type": "text"}, "verbosity": "medium"}, "tool_choice": "auto", "tools": body.get("tools") or [], "top_p": 1.0,
                "truncation": "disabled", "usage": {"input_tokens": it, "input_tokens_details": {"cached_tokens": 0}, "output_tokens": ot,
                                                    "output_tokens_details": {"reasoning_tokens": 0}, "total_tokens": it + ot},
                "user": None, "metadata": {}}
        if not body.get("stream"):
            return self.js(resp)
        ev = []
        seq = [0]

        def e(obj):
            obj["sequence_number"] = seq[0]
            seq[0] += 1
            ev.append("event: %s\ndata: %s\n\n" % (obj["type"], json.dumps(obj)))

        start = dict(resp, status="in_progress", output=[], usage=None)
        e({"type": "response.created", "response": start})
        e({"type": "response.in_progress", "response": start})
        for i, o in enumerate(output):
            if o["type"] == "message":
                e({"type": "response.output_item.added", "output_index": i, "item": dict(o, status="in_progress", content=[])})
                part = {"type": "output_text", "text": "", "annotations": [], "logprobs": []}
                e({"type": "response.content_part.added", "item_id": o["id"], "output_index": i, "content_index": 0, "part": part})
                words = o["content"][0]["text"].split(" ")
                for k in range(0, len(words), 2):
                    e({"type": "response.output_text.delta", "item_id": o["id"], "output_index": i, "content_index": 0,
                       "delta": " ".join(words[k:k + 2]) + (" " if k + 2 < len(words) else ""), "logprobs": []})
                e({"type": "response.output_text.done", "item_id": o["id"], "output_index": i, "content_index": 0, "text": o["content"][0]["text"], "logprobs": []})
                e({"type": "response.content_part.done", "item_id": o["id"], "output_index": i, "content_index": 0, "part": o["content"][0]})
            else:
                e({"type": "response.output_item.added", "output_index": i, "item": dict(o, status="in_progress", arguments="")})
                e({"type": "response.function_call_arguments.delta", "item_id": o["id"], "output_index": i, "delta": o["arguments"]})
                e({"type": "response.function_call_arguments.done", "item_id": o["id"], "output_index": i, "name": o["name"], "arguments": o["arguments"]})
            e({"type": "response.output_item.done", "output_index": i, "item": o})
        e({"type": "response.completed", "response": resp})
        self.sse(ev)


class Srv(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18090
    s = Srv(("127.0.0.1", port), H)
    print("listening", port, flush=True)
    s.serve_forever()
