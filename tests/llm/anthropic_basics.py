# anthropic: Messages, tool use, errors, retries, count_tokens, models, and what a program sees of the results
# (repr, attributes, to_dict / to_json / model_dump, request ids). Talks to tests/llm/server.py.
import json
import anthropic
from anthropic import Anthropic
from anthropic.types import Message, TextBlock, ToolUseBlock

B = "http://127.0.0.1:18090"
client = Anthropic(api_key="test-key", base_url=B)

m = client.messages.create(model="claude-mock-sonnet", max_tokens=100, messages=[{"role": "user", "content": "Hello there"}])
print(m)
print(type(m).__name__, isinstance(m, Message), isinstance(m.content[0], TextBlock))
print(m.id, m.role, m.model, m.stop_reason, m.stop_sequence, m.type)
print(m.content[0].text, m.content[0].type, m.content[0].citations)
print(m.usage)
print(m.usage.input_tokens, m.usage.output_tokens, m._request_id)
print(m.to_dict())
print(m.to_json())
print(m.model_dump())
print(m.model_dump(exclude_none=True))
print(m.to_json(indent=None))
print(m.model_dump_json())
print(m.usage.model_dump(), m.content[0].to_dict())
print(m == client.messages.create(model="claude-mock-sonnet", max_tokens=100, messages=[{"role": "user", "content": "Hello there"}]))
print(dict(m.usage))

# system prompt, stop sequences, max_tokens, conversation
m = client.messages.create(
    model="claude-mock-opus",
    max_tokens=3,
    system="Be brief.",
    messages=[{"role": "user", "content": [{"type": "text", "text": "one two three four five"}]}],
)
print(m.content[0].text, "|", m.stop_reason)
m = client.messages.create(model="claude-mock-opus", max_tokens=50, stop_sequences=["three"], messages=[{"role": "user", "content": "one two three four"}],
                           metadata={"user_id": "u-1"})
print(repr(m.content[0].text), m.stop_reason, m.stop_sequence)
convo = [{"role": "user", "content": "Hi"}]
for turn in range(3):
    r = client.messages.create(model="claude-mock-haiku", max_tokens=64, messages=convo)
    print(turn, r.content[0].text)
    convo.append({"role": "assistant", "content": r.content})
    convo.append({"role": "user", "content": "again %d" % turn})

# tool use: an agent loop that sends the SDK's own blocks back
tools = [
    {
        "name": "get_weather",
        "description": "Weather for a city",
        "input_schema": {"type": "object", "properties": {"city": {"type": "string"}, "days": {"type": "integer"}, "metric": {"type": "boolean"}}, "required": ["city"]},
    },
    {"name": "noop", "description": "does nothing", "input_schema": {"type": "object", "properties": {}}},
]


def run_tool(name, args):
    if name == "get_weather":
        return json.dumps({"city": args["city"], "temp": 21.5, "days": args.get("days")})
    return "nothing"


msgs = [{"role": "user", "content": "Weather in Paris?"}]
while True:
    r = client.messages.create(model="claude-mock-sonnet", max_tokens=200, tools=tools, messages=msgs)
    print(r.stop_reason, [b.type for b in r.content])
    if r.stop_reason != "tool_use":
        print(r.content[0].text)
        break
    msgs.append({"role": "assistant", "content": r.content})
    results = []
    for b in r.content:
        if isinstance(b, ToolUseBlock):
            print(b)
            print(b.name, b.input, b.id, type(b.input).__name__)
            results.append({"type": "tool_result", "tool_use_id": b.id, "content": run_tool(b.name, b.input)})
    msgs.append({"role": "user", "content": results})
print(len(msgs))
r = client.messages.create(model="claude-mock-sonnet", max_tokens=200, tools=tools, tool_choice={"type": "tool", "name": "noop"}, messages=[{"role": "user", "content": "x"}])
print(r.content[1].name, r.content[1].input, r.to_dict()["content"][1])
r = client.messages.create(model="claude-mock-sonnet", max_tokens=200, tools=tools, tool_choice={"type": "none"}, messages=[{"role": "user", "content": "plain"}])
print(r.content[0].text)
r = client.messages.create(model="claude-mock-sonnet", max_tokens=2000, thinking={"type": "adaptive"}, messages=[{"role": "user", "content": "think"}])
print(r.content)

# errors
for model in ["mock-error-400", "mock-error-404", "mock-error-422", "mock-error-429", "mock-error-500", "mock-error-529", "mock-error-403", "mock-error-409", "mock-error-413"]:
    try:
        client.with_options(max_retries=0).messages.create(model=model, max_tokens=10, messages=[{"role": "user", "content": "x"}])
    except anthropic.APIStatusError as e:
        print(type(e).__name__, e.status_code, e.message)
        print(" ", e.body, e.request_id, e.type, e.api_message, e.error_code, e.details, isinstance(e, anthropic.APIError), e.response.status_code)
try:
    Anthropic(api_key="wrong", base_url=B).messages.create(model="claude-mock-opus", max_tokens=10, messages=[{"role": "user", "content": "x"}])
except anthropic.AuthenticationError as e:
    print("auth:", e)
try:
    Anthropic(api_key="test-key", base_url="http://127.0.0.1:18099", max_retries=0).messages.create(model="m", max_tokens=10, messages=[])
except anthropic.APIConnectionError as e:
    print(type(e).__name__, e, e.message, e.body)
try:
    client.messages.create(model="claude-mock-opus", max_tokens=10)
except TypeError as e:
    print("TypeError", e)
try:
    client.messages.create(model="claude-mock-opus", max_tokens=200000, messages=[{"role": "user", "content": "x"}])
except ValueError as e:
    print("ValueError", str(e)[:60])

# retries: the first try of mock-flaky fails with 529, the retry succeeds
r = client.messages.create(model="mock-flaky", max_tokens=20, messages=[{"role": "user", "content": "retry me"}])
print(r.content[0].text, r.model)
try:
    client.with_options(max_retries=1).messages.create(model="mock-error-429", max_tokens=10, messages=[{"role": "user", "content": "x"}])
except anthropic.RateLimitError as e:
    print("RateLimitError after retry", e.status_code)

# count tokens, models
print(client.messages.count_tokens(model="claude-mock-opus", messages=[{"role": "user", "content": "count these words"}]))
page = client.models.list(limit=2)
print(page)
print([m.id for m in page.data], page.has_more, page.last_id)
print([m.id for m in client.models.list(limit=1)])
info = client.models.retrieve("claude-mock-haiku")
print(info)
print(info.created_at.year, info.display_name, info.to_dict())
try:
    client.models.retrieve("nope")
except anthropic.NotFoundError as e:
    print("NotFoundError", e.status_code)

# what was sent
c2 = Anthropic(api_key="test-key", base_url=B + "/v0", default_headers={"X-Custom": "yes"})
print(c2.base_url, c2.max_retries, c2.api_key, c2.timeout)
print(anthropic.__version__, anthropic.DEFAULT_MAX_RETRIES)
client.close()
with Anthropic(api_key="test-key", base_url=B) as c3:
    print(c3.messages.create(model="claude-mock-haiku", max_tokens=5, messages=[{"role": "user", "content": "ctx"}]).content[0].text)
