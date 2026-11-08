# anthropic structured outputs: messages.parse(output_format=Model) (the request's output_config.format is the
# model's JSON schema, transformed as the SDK does), parsed_output on text blocks, output_config passed by hand,
# count_tokens and messages.stream with output_format. Talks to tests/llm/server.py.
import json
from enum import Enum
from typing import Optional

from anthropic import Anthropic
from pydantic import BaseModel, Field

B = "http://127.0.0.1:18090"
client = Anthropic(api_key="test-key", base_url=B)


class Mood(str, Enum):
    happy = "happy"
    sad = "sad"


class Contact(BaseModel):
    """Someone to write to."""

    name: str = Field(description="Full name")
    email: str
    age: int = Field(ge=0, le=150)
    mood: Mood
    tags: list[str] = []
    note: Optional[str] = None


class Book(BaseModel):
    title: str
    authors: list[Contact]
    pages: int


m = client.messages.parse(model="claude-mock-sonnet", max_tokens=200, messages=[{"role": "user", "content": "Ada Lovelace, ada@example.com"}],
                          output_format=Contact)
print(type(m).__name__, m.stop_reason)
print(type(m.content[0]).__name__, m.content[0].text)
c = m.content[0].parsed_output
print(type(c).__name__, c)
print(c.name, c.age, c.mood, c.tags, c.note)
print(m.parsed_output == c)
print(m)
print(m.to_dict()['content'])
print(m.content[0].model_dump())
print(m.content[0].to_json(indent=None))

b = client.messages.parse(model="claude-mock-opus", max_tokens=300, messages=[{"role": "user", "content": "A book"}], output_format=Book,
                          system="Answer in JSON.")
print(b.parsed_output)
print(b.parsed_output.authors[0].mood is Mood.happy)

# a list type, not a model
nums = client.messages.parse(model="claude-mock-haiku", max_tokens=50, messages=[{"role": "user", "content": "three numbers"}],
                             output_format=list[int])
print(nums.parsed_output, nums.content[0].text)

# output_config by hand: create() leaves the text alone
raw = client.messages.create(model="claude-mock-haiku", max_tokens=50, messages=[{"role": "user", "content": "x"}],
                             output_config={"format": {"type": "json_schema", "schema": {"type": "object", "properties": {"ok": {"type": "boolean"}},
                                                                                         "required": ["ok"], "additionalProperties": False}}})
print(json.loads(raw.content[0].text))

# without output_format, parse() gives parsed_output None
p = client.messages.parse(model="claude-mock-haiku", max_tokens=50, messages=[{"role": "user", "content": "plain"}])
print(p.content[0].text, p.content[0].parsed_output, p.parsed_output)

try:
    client.messages.parse(model="claude-mock-haiku", max_tokens=50, messages=[{"role": "user", "content": "x"}], output_format={"type": "object"})
except TypeError as e:
    print("TypeError:", e)

print(client.messages.count_tokens(model="claude-mock-haiku", messages=[{"role": "user", "content": "count me"}], output_format=Contact).input_tokens)

with client.messages.stream(model="claude-mock-sonnet", max_tokens=200, messages=[{"role": "user", "content": "Grace Hopper"}],
                            output_format=Contact) as s:
    for t in s.text_stream:
        pass
    f = s.get_final_message()
print(type(f).__name__, f.parsed_output)

# a type alias as output_format
Scores = Optional[list[float]]
sc = client.messages.parse(model="claude-mock-haiku", max_tokens=50, messages=[{"role": "user", "content": "scores"}], output_format=Scores)
print(sc.content[0].text, sc.parsed_output)
