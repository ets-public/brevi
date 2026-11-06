# openai: AsyncOpenAI on asyncio: chat completions, gather, streaming, embeddings, responses, errors.
import asyncio
import openai
from openai import AsyncOpenAI


async def ask(client, text):
    r = await client.chat.completions.create(model="gpt-mock", messages=[{"role": "user", "content": text}])
    return r.choices[0].message.content


async def main():
    client = AsyncOpenAI(api_key="test-key", base_url="http://127.0.0.1:18090/openai/v1", max_retries=0)
    r = await client.chat.completions.create(model="gpt-mock", messages=[{"role": "user", "content": "async hello"}])
    print(r)
    print(await asyncio.gather(*[ask(client, "q%d" % i) for i in range(4)]))
    stream = await client.chat.completions.create(model="gpt-mock", messages=[{"role": "user", "content": "a b c d"}], stream=True)
    print(type(stream).__name__)
    async for chunk in stream:
        print(chunk.choices[0].delta.content, chunk.choices[0].finish_reason)
    e = await client.embeddings.create(model="m", input="vec")
    print(e.data[0].embedding)
    resp = await client.responses.create(model="gpt-mock", input="async responses")
    print(resp.output_text)
    async for ev in await client.responses.create(model="gpt-mock", input="s", stream=True):
        print(ev.type)
    ms = await client.models.list()
    print([m.id for m in ms.data])
    ids = [m.id async for m in client.models.list()]
    print(ids)
    try:
        await client.chat.completions.create(model="mock-error-429", messages=[{"role": "user", "content": "x"}])
    except openai.RateLimitError as e:
        print("RateLimitError", e.status_code, e.code)
    r = await client.with_options(max_retries=2).chat.completions.create(model="mock-flaky", messages=[{"role": "user", "content": "f"}])
    print(r.choices[0].message.content)
    await client.close()
    async with AsyncOpenAI(api_key="test-key", base_url="http://127.0.0.1:18090/openai/v1") as c:
        print(await ask(c, "with"))


asyncio.run(main())
