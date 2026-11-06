import asyncio

async def job(n, d):
    await asyncio.sleep(d)
    if n == 3:
        raise KeyError(n)
    return n * 10

async def main():
    tasks = [asyncio.create_task(job(i, 0.01 * i)) for i in (1, 2, 4)]
    done, pending = await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
    print(sorted(t.result() for t in done), len(pending))
    done, pending = await asyncio.wait(pending)
    print(sorted(t.result() for t in done), len(pending))
    tasks = [asyncio.create_task(job(i, 0.01 * i)) for i in (3, 5)]
    done, pending = await asyncio.wait(tasks, return_when=asyncio.FIRST_EXCEPTION)
    print([repr(t.exception()) for t in done], len(pending))
    for t in pending:
        t.cancel()
    tasks = [asyncio.create_task(job(i, 0.05)) for i in (6,)]
    done, pending = await asyncio.wait(tasks, timeout=0.01)
    print(len(done), len(pending))
    await asyncio.wait(pending)
    out = []
    for c in asyncio.as_completed([job(7, 0.03), job(8, 0.01), job(9, 0.02)]):
        out.append(await c)
    print(out)
    out = []
    async for t in asyncio.as_completed([job(10, 0.02), job(11, 0.01)]):
        out.append(t.result())
    print(out)
    try:
        await asyncio.wait([])
    except ValueError as e:
        print(e)

asyncio.run(main())
