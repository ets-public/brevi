# openai structured outputs: chat.completions.parse / .stream and responses.parse / .stream with a pydantic model
# (the request's response_format / text.format is the model's strict JSON schema, made as the SDK makes it),
# pydantic_function_tool and parsed tool arguments, the stream helpers' events. Talks to tests/llm/server.py.
import asyncio
from enum import Enum
from typing import Optional

import openai
from openai import AsyncOpenAI, OpenAI
from pydantic import BaseModel, Field

B = "http://127.0.0.1:18090/openai/v1"
client = OpenAI(api_key="test-key", base_url=B)


class Color(str, Enum):
    red = "red"
    blue = "blue"


class Step(BaseModel):
    explanation: str
    output: str


class Math(BaseModel):
    """A worked answer."""

    steps: list[Step]
    final_answer: str = Field(description="The answer")
    confidence: float
    color: Color
    note: Optional[str] = None


class GetWeather(BaseModel):
    """Get the weather in a city."""

    city: str
    days: int = 1


print(openai.pydantic_function_tool(GetWeather))
print(openai.pydantic_function_tool(GetWeather, name="weather", description="Weather now"))

c = client.chat.completions.parse(model="gpt-mock", messages=[{"role": "user", "content": "solve 8x + 31 = 2"}], response_format=Math)
print(type(c).__name__, type(c.choices[0]).__name__, type(c.choices[0].message).__name__)
m = c.choices[0].message
print(m.content)
print(type(m.parsed).__name__, m.parsed)
print(m.parsed.steps[0].explanation, m.parsed.color is Color.red, m.refusal)
print(c)
print(c.to_dict()["choices"][0]["message"])

p = client.chat.completions.parse(model="gpt-mock", messages=[{"role": "user", "content": "plain"}])
print(p.choices[0].message.content, p.choices[0].message.parsed)

t = client.chat.completions.parse(model="gpt-mock", messages=[{"role": "user", "content": "weather in Oslo"}],
                                  tools=[openai.pydantic_function_tool(GetWeather)], parallel_tool_calls=False)
call = t.choices[0].message.tool_calls[0]
print(type(call).__name__, type(call.function).__name__, call.function.name, call.function.arguments)
print(type(call.function.parsed_arguments).__name__, call.function.parsed_arguments)

try:
    client.chat.completions.parse(model="gpt-mock", messages=[{"role": "user", "content": "x"}], response_format={"type": "json_object"})
except TypeError as e:
    print("TypeError:", e)

with client.chat.completions.stream(model="gpt-mock", messages=[{"role": "user", "content": "stream it"}], response_format=Math) as s:
    kinds = []
    for ev in s:
        kinds.append(ev.type)
        if ev.type == "content.done":
            print("done", type(ev).__name__, ev.parsed)
    print(kinds)
    f = s.get_final_completion()
print(type(f).__name__, f.choices[0].message.parsed)

with client.chat.completions.stream(model="gpt-mock", messages=[{"role": "user", "content": "plain words here"}]) as s:
    for ev in s:
        if ev.type == "content.delta":
            print(repr(ev.delta), repr(ev.snapshot), ev.parsed)
        elif ev.type == "content.done":
            print("content.done", repr(ev.content), ev.parsed)
        elif ev.type == "chunk":
            pass
        else:
            print(ev.type)
    print(s.current_completion_snapshot.choices[0].message.content)

with client.chat.completions.stream(model="gpt-mock", messages=[{"role": "user", "content": "weather in Rome"}],
                                    tools=[openai.pydantic_function_tool(GetWeather)], parallel_tool_calls=False) as s:
    for ev in s:
        if ev.type == "tool_calls.function.arguments.delta":
            print(ev.name, ev.index, repr(ev.arguments_delta), repr(ev.arguments), ev.parsed_arguments)
        elif ev.type == "tool_calls.function.arguments.done":
            print("args.done", ev.name, ev.index, ev.arguments, ev.parsed_arguments)
    f = s.get_final_completion()
print(f.choices[0].message.tool_calls[0].function.parsed_arguments)

# Responses
r = client.responses.parse(model="gpt-mock", input="solve 2x = 4", text_format=Math)
print(type(r).__name__, type(r.output[0]).__name__, type(r.output[0].content[0]).__name__)
print(r.output_text)
print(type(r.output_parsed).__name__, r.output_parsed)
print(r.output[0].content[0].parsed == r.output_parsed)
print(r.text)

tool = {"type": "function", "name": "get_weather", "description": "Weather", "strict": True,
        "parameters": {"type": "object", "properties": {"city": {"type": "string"}}, "required": ["city"], "additionalProperties": False}}
rt = client.responses.parse(model="gpt-mock", input="weather in Lima", tools=[tool])
print(type(rt.output[0]).__name__, rt.output[0].name, rt.output[0].arguments, rt.output[0].parsed_arguments)

pr = client.responses.parse(model="gpt-mock", input="hi there")
print(pr.output_text, pr.output_parsed)

with client.responses.stream(model="gpt-mock", input="stream a response", text_format=Math) as s:
    kinds = []
    for ev in s:
        kinds.append(ev.type)
        if ev.type == "response.output_text.delta":
            print(repr(ev.delta), repr(ev.snapshot))
    print(kinds)
    fr = s.get_final_response()
print(type(fr).__name__, fr.output_parsed)

with client.responses.stream(model="gpt-mock", input="say words") as s:
    for ev in s:
        if ev.type == "response.output_text.delta":
            print(repr(ev.delta), repr(ev.snapshot))
        elif ev.type == "response.completed":
            print(ev.response.output_text)
    print(s.get_final_response().output_parsed)


async def main():
    a = AsyncOpenAI(api_key="test-key", base_url=B)
    c = await a.chat.completions.parse(model="gpt-mock", messages=[{"role": "user", "content": "async solve"}], response_format=Step)
    print(c.choices[0].message.parsed)
    async with a.chat.completions.stream(model="gpt-mock", messages=[{"role": "user", "content": "async stream"}], response_format=Step) as s:
        async for ev in s:
            if ev.type == "content.done":
                print("async done", ev.parsed)
        print((await s.get_final_completion()).choices[0].message.parsed)
    r = await a.responses.parse(model="gpt-mock", input="async response", text_format=Step)
    print(r.output_parsed)
    async with a.responses.stream(model="gpt-mock", input="async response stream", text_format=Step) as s:
        async for ev in s:
            pass
        print((await s.get_final_response()).output_parsed)


asyncio.run(main())
