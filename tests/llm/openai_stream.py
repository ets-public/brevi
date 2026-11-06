# openai: streaming chat completions (text, usage, tool call deltas), the Responses API's stream events, and an
# error in the middle of a stream.
import openai

client = openai.OpenAI(api_key="test-key", base_url="http://127.0.0.1:18090/openai/v1", max_retries=0)
msgs = [{"role": "user", "content": "stream these words to me"}]

stream = client.chat.completions.create(model="gpt-mock", messages=msgs, stream=True)
print(type(stream).__name__)
for chunk in stream:
    print(chunk)
text = ""
for chunk in client.chat.completions.create(model="gpt-mock", messages=msgs, stream=True, stream_options={"include_usage": True}):
    if chunk.choices and chunk.choices[0].delta.content:
        text += chunk.choices[0].delta.content
    if chunk.usage:
        print("usage", chunk.usage.total_tokens, chunk.choices)
print(repr(text))

tools = [{"type": "function", "function": {"name": "lookup", "parameters": {"type": "object", "properties": {"word": {"type": "string"}, "n": {"type": "integer"}}}}}]
calls = {}
with client.chat.completions.create(model="gpt-mock", messages=msgs, tools=tools, stream=True) as s:
    for chunk in s:
        d = chunk.choices[0].delta
        for tc in d.tool_calls or []:
            print(tc)
            c = calls.setdefault(tc.index, {"id": None, "name": None, "args": ""})
            if tc.id:
                c["id"] = tc.id
            if tc.function.name:
                c["name"] = tc.function.name
            c["args"] += tc.function.arguments or ""
        if chunk.choices[0].finish_reason:
            print("finish", chunk.choices[0].finish_reason)
print(calls)

for ev in client.responses.create(model="gpt-mock", input="hi there friend", stream=True):
    print(ev.type, ev.sequence_number)
    if ev.type == "response.output_text.delta":
        print(repr(ev.delta))
    if ev.type == "response.completed":
        print(ev.response.output_text, ev.response.usage)
    if ev.type == "response.output_item.done":
        print(ev.item)

try:
    for chunk in client.chat.completions.create(model="mock-stream-error", messages=msgs, stream=True):
        print("got", chunk.choices[0].delta.content)
except openai.APIError as e:
    print(type(e).__name__, e.message, e.body)
