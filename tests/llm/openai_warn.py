# pydantic's serializer warnings on openai's parsed models: `parsed` is Optional[ResponseFormatT] (a TypeVar that
# defaults to None), so dumping a parse() result that holds a model warns "Expected `none`" from pydantic/main.py
# on stderr (openai_warn.stderr), once per text and line. No warnings import and no `warnings=` argument: a
# program with either runs openai and pydantic in CPython (caps.rs). Talks to tests/llm/server.py.
import sys

from openai import OpenAI
from pydantic import BaseModel

client = OpenAI(api_key="test-key", base_url="http://127.0.0.1:18090/openai/v1")


class Step(BaseModel):
    explanation: str
    output: str


class Long(BaseModel):
    text: str


def show(label, f):
    print("==", label, file=sys.stderr, flush=True)
    try:
        print(label, type(f()).__name__)
    except Exception as e:
        print(label, type(e).__name__, type(e).__module__, e)


c = client.chat.completions.parse(model="gpt-mock", messages=[{"role": "user", "content": "solve"}], response_format=Step)
show("to_dict", c.to_dict)
show("to_dict again", c.to_dict)
show("to_json", c.to_json)
show("model_dump json", lambda: c.model_dump(mode="json"))
show("model_dump_json", c.model_dump_json)
for w in (False, "none", "error", "bad", 0, None):
    kw = {"warnings": w}
    show("warnings " + repr(w), lambda: c.model_dump(**kw))
show("exclude parsed", lambda: c.choices[0].message.model_dump(exclude={"parsed"}))
show("include content", lambda: c.choices[0].message.model_dump(include={"content"}))
p = client.chat.completions.parse(model="gpt-mock", messages=[{"role": "user", "content": "plain"}])
show("no format", p.to_dict)
m = client.chat.completions.parse(model="gpt-mock", messages=[{"role": "user", "content": "other"}], response_format=Step)
show("message", m.choices[0].message.to_dict)
show("its completion", m.to_dict)
g = client.chat.completions.parse(model="gpt-mock", messages=[{"role": "user", "content": "réponse très élevée, ça " * 3}], response_format=Long)
show("long", g.to_dict)
r = client.responses.parse(model="gpt-mock", input="solve 2x = 4", text_format=Step)
show("responses", r.to_dict)
with client.chat.completions.stream(model="gpt-mock", messages=[{"role": "user", "content": "stream it"}], response_format=Step) as s:
    for ev in s:
        if ev.type in ("content.delta", "content.done"):
            show(ev.type, ev.model_dump)
    f = s.get_final_completion()
show("final", f.to_dict)
with client.responses.stream(model="gpt-mock", input="stream a response", text_format=Step) as s:
    for ev in s:
        if ev.type == "response.output_text.done":
            show(ev.type, ev.to_dict)
print("done")
