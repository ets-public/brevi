# anthropic: AsyncAnthropic on asyncio: create, gather, streaming (both ways), errors, models paging.
import asyncio
import anthropic
from anthropic import AsyncAnthropic


async def ask(client, text):
    m = await client.messages.create(model="claude-mock-haiku", max_tokens=50, messages=[{"role": "user", "content": text}])
    return m.content[0].text


async def main():
    client = AsyncAnthropic(api_key="test-key", base_url="http://127.0.0.1:18090", max_retries=0)
    m = await client.messages.create(model="claude-mock-sonnet", max_tokens=50, messages=[{"role": "user", "content": "async hello"}])
    print(m)
    print(m._request_id)
    print(await asyncio.gather(*[ask(client, "q%d" % i) for i in range(5)]))
    stream = await client.messages.create(model="claude-mock-sonnet", max_tokens=50, messages=[{"role": "user", "content": "a b c d e"}], stream=True)
    print(type(stream).__name__)
    async for ev in stream:
        print(ev.type)
    async with client.messages.stream(model="claude-mock-sonnet", max_tokens=50, messages=[{"role": "user", "content": "one two three"}]) as s:
        async for t in s.text_stream:
            print(repr(t))
        final = await s.get_final_message()
        print(final.content[0].text, type(final).__name__)
        print(await s.get_final_text())
    async with client.messages.stream(model="claude-mock-sonnet", max_tokens=50, messages=[{"role": "user", "content": "one"}]) as s:
        async for ev in s:
            print(ev.type)
    try:
        await client.messages.create(model="mock-error-400", max_tokens=5, messages=[{"role": "user", "content": "x"}])
    except anthropic.BadRequestError as e:
        print("BadRequestError", e.status_code, e.body["error"]["message"])
    r = await client.with_options(max_retries=2).messages.create(model="mock-flaky", max_tokens=20, messages=[{"role": "user", "content": "flaky"}], extra_headers={"X-Custom": "1"})
    print(r.content[0].text)
    print(await client.messages.count_tokens(model="m", messages=[{"role": "user", "content": "abc"}]))
    page = await client.models.list(limit=1)
    print([x.id for x in page.data], page.has_more)
    ids = []
    async for x in client.models.list(limit=2):
        ids.append(x.id)
    print(ids)
    print((await client.models.retrieve("claude-mock-opus")).display_name)
    await client.close()
    async with AsyncAnthropic(api_key="test-key", base_url="http://127.0.0.1:18090") as c:
        print(await ask(c, "with"))
    try:
        await AsyncAnthropic(api_key="test-key", base_url="http://127.0.0.1:18099", max_retries=0).messages.create(model="m", max_tokens=1, messages=[])
    except anthropic.APIConnectionError as e:
        print(type(e).__name__, e)


asyncio.run(main())
