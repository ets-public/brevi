import asyncio
async def w(i, d):
    await asyncio.sleep(d)
    print(i, "a")
    await asyncio.sleep(0)
    print(i, "b")
async def main():
    await asyncio.gather(*(w(i, 0.01) for i in range(5)))
    ts = [asyncio.create_task(w(i, 0.02)) for i in range(5, 10)]
    for t in ts:
        await t
    async with asyncio.TaskGroup() as tg:
        for i in range(10, 14):
            tg.create_task(w(i, 0.005))
asyncio.run(main())
