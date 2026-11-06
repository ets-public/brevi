# anthropic: streaming. create(stream=True) events, the messages.stream() helper (text_stream, events,
# get_final_message, get_final_text), tool input JSON arriving in pieces, thinking, and an error mid-stream.
import anthropic

client = anthropic.Anthropic(api_key="test-key", base_url="http://127.0.0.1:18090", max_retries=0)
msgs = [{"role": "user", "content": "stream these words to me please"}]

stream = client.messages.create(model="claude-mock-sonnet", max_tokens=100, messages=msgs, stream=True)
print(type(stream).__name__)
for event in stream:
    print(event.type, event)
print(stream.response.status_code, stream.response.headers["content-type"])

with client.messages.create(model="claude-mock-sonnet", max_tokens=100, messages=msgs, stream=True) as s:
    text = ""
    for event in s:
        if event.type == "content_block_delta" and event.delta.type == "text_delta":
            text += event.delta.text
    print(repr(text))

with client.messages.stream(model="claude-mock-opus", max_tokens=100, messages=msgs) as s:
    for t in s.text_stream:
        print(repr(t))
    final = s.get_final_message()
    print(final)
    print(type(final).__name__, final.content[0].text, final.usage.output_tokens, s.request_id)
    print(s.get_final_text())

with client.messages.stream(model="claude-mock-opus", max_tokens=100, messages=msgs) as s:
    for event in s:
        print(event.type, event)

tools = [{"name": "lookup", "description": "Look up a word", "input_schema": {"type": "object", "properties": {"word": {"type": "string"}, "limit": {"type": "integer"}, "tags": {"type": "array"}}}}]
with client.messages.stream(model="claude-mock-opus", max_tokens=100, messages=[{"role": "user", "content": "define serendipity"}], tools=tools) as s:
    for event in s:
        if event.type == "input_json":
            print("json", repr(event.partial_json), event.snapshot)
        elif event.type in ("text", "content_block_stop", "message_stop"):
            print(event.type, event)
    m = s.get_final_message()
    print(m.stop_reason, m.content[1].input, m.content[1].name)
    try:
        print(s.get_final_text())
    except RuntimeError as e:
        print("RuntimeError", e)

with client.messages.stream(model="claude-mock-opus", max_tokens=2000, messages=msgs, thinking={"type": "enabled", "budget_tokens": 1024}) as s:
    for event in s:
        if event.type in ("thinking", "signature", "text"):
            print(event)
    print(s.get_final_message().content)

s = client.messages.stream(model="claude-mock-haiku", max_tokens=100, messages=msgs)
with s as st:
    st.until_done()
    print(st.current_message_snapshot.content[0].text)

try:
    for event in client.messages.create(model="mock-stream-error", max_tokens=100, messages=msgs, stream=True):
        print("got", event.type)
except anthropic.APIStatusError as e:
    print(type(e).__name__, e.status_code, e.message, e.body)

try:
    with client.messages.stream(model="mock-error-429", max_tokens=100, messages=msgs) as s:
        print("no")
except anthropic.RateLimitError as e:
    print("RateLimitError", e.status_code)
