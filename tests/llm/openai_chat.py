# openai: chat completions (tools, an agent loop sending the SDK's own messages back), embeddings (the SDK's
# default base64 and plain floats), models, the Responses API, errors and retries, and what a program sees of
# the results. Talks to tests/llm/server.py.
import json
import openai
from openai import OpenAI
from openai.types.chat import ChatCompletion, ChatCompletionMessage

B = "http://127.0.0.1:18090/openai/v1"
client = OpenAI(api_key="test-key", base_url=B)

c = client.chat.completions.create(model="gpt-mock", messages=[{"role": "user", "content": "Hello there"}])
print(c)
print(type(c).__name__, isinstance(c, ChatCompletion), isinstance(c.choices[0].message, ChatCompletionMessage))
print(c.choices[0].message.content, c.choices[0].finish_reason, c.usage.total_tokens, c.id, c.created, c._request_id)
print(c.to_dict())
print(c.to_json())
print(c.model_dump(exclude_none=True))
print(c.choices[0].message.model_dump_json())
c = client.chat.completions.create(model="gpt-mock", messages=[{"role": "system", "content": "Be brief."}, {"role": "user", "content": "one two three four"}],
                                   max_completion_tokens=3, temperature=0.2, n=2, stop=["four"], seed=7, user="u1")
print([ch.message.content for ch in c.choices], [ch.finish_reason for ch in c.choices])

tools = [
    {"type": "function", "function": {"name": "get_weather", "description": "Weather", "parameters": {"type": "object", "properties": {"city": {"type": "string"}, "days": {"type": "integer"}}, "required": ["city"]}}},
    {"type": "function", "function": {"name": "get_time", "description": "Time", "parameters": {"type": "object", "properties": {"zone": {"type": "string", "enum": ["UTC", "CET"]}}}}},
]
msgs = [{"role": "user", "content": "Weather and time in Paris?"}]
while True:
    r = client.chat.completions.create(model="gpt-mock", messages=msgs, tools=tools, tool_choice="auto")
    m = r.choices[0].message
    print(r.choices[0].finish_reason, m.content, m.tool_calls and len(m.tool_calls))
    if not m.tool_calls:
        break
    msgs.append(m)
    for tc in m.tool_calls:
        print(tc)
        args = json.loads(tc.function.arguments)
        msgs.append({"role": "tool", "tool_call_id": tc.id, "content": json.dumps({"tool": tc.function.name, "args": args})})
print(m.content)
r = client.chat.completions.create(model="gpt-mock", messages=[{"role": "user", "content": "x"}], tools=tools, parallel_tool_calls=False)
print(len(r.choices[0].message.tool_calls), r.choices[0].message.to_dict())

e = client.embeddings.create(model="text-embedding-mock", input="hello world")
print(e)
print(len(e.data[0].embedding), e.data[0].embedding[:3], e.usage)
e = client.embeddings.create(model="text-embedding-mock", input=["a", "b c"], encoding_format="float", dimensions=3)
print([d.embedding for d in e.data], [d.index for d in e.data])
e = client.embeddings.create(model="text-embedding-mock", input=["a", "b c"], encoding_format="base64", dimensions=2)
print([d.embedding for d in e.data])

ms = client.models.list()
print(ms)
print([m.id for m in ms], [m.owned_by for m in ms.data])

r = client.responses.create(model="gpt-mock", input="Tell me a joke", instructions="Be funny")
print(r.output_text)
print(r.id, r.status, r.usage.total_tokens, r.output[0].type, r.output[0].content[0].text)
print(r.output[0])
r = client.responses.create(model="gpt-mock", input=[{"role": "user", "content": "Weather?"}],
                            tools=[{"type": "function", "name": "get_weather", "description": "w", "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}}])
fc = r.output[0]
print(fc.type, fc.name, fc.arguments, fc.call_id, repr(r.output_text))
r2 = client.responses.create(model="gpt-mock", previous_response_id=r.id,
                             input=[{"type": "function_call_output", "call_id": fc.call_id, "output": "sunny"}])
print(r2.output_text, r2.previous_response_id)

for model in ["mock-error-400", "mock-error-401", "mock-error-404", "mock-error-409", "mock-error-422", "mock-error-429", "mock-error-500", "mock-error-503"]:
    try:
        client.with_options(max_retries=0).chat.completions.create(model=model, messages=[{"role": "user", "content": "x"}])
    except openai.APIStatusError as e:
        print(type(e).__name__, e.status_code, e.message)
        print(" ", e.body, e.code, e.param, e.type, e.request_id, isinstance(e, openai.APIError))
try:
    OpenAI(api_key="wrong", base_url=B).chat.completions.create(model="gpt-mock", messages=[])
except openai.AuthenticationError as e:
    print("auth:", e)
try:
    OpenAI(api_key="test-key", base_url="http://127.0.0.1:18099/v1", max_retries=0).models.list()
except openai.APIConnectionError as e:
    print(type(e).__name__, e)
r = client.chat.completions.create(model="mock-flaky", messages=[{"role": "user", "content": "retry"}])
print(r.choices[0].message.content)
try:
    OpenAI(base_url=B, api_key=None)
except openai.OpenAIError as e:
    print("OpenAIError", e)
print(openai.__version__, client.max_retries, client.base_url, client.api_key, client.organization, client.project)
with OpenAI(api_key="test-key", base_url=B, organization="org-1", project="proj-1") as c2:
    print(c2.chat.completions.create(model="gpt-mock", messages=[{"role": "developer", "content": "dev"}, {"role": "user", "content": "ctx"}]).choices[0].message.content)
