import asyncio

log = []
async def a():
    for i in range(3):
        print("a", i)
        await asyncio.sleep(0)

async def b():
    for i in range(3):
        print("b", i)
        await asyncio.sleep(0)
        await asyncio.sleep(0)

async def waits(f, name):
    print(name, "waits")
    v = await f
    print(name, "got", v)

async def main():
    t1 = asyncio.create_task(a())
    t2 = asyncio.create_task(b())
    await asyncio.sleep(0)
    print("main after first sleep0")
    loop = asyncio.get_running_loop()
    f = loop.create_future()
    w = [asyncio.create_task(waits(f, f"w{i}")) for i in range(3)]
    await asyncio.sleep(0)
    f.set_result("R")
    print("set result")
    loop.call_soon(lambda: print("soon cb"))
    await asyncio.gather(t1, t2, *w)
    print("all done")
    async def bg():
        try:
            print("bg start")
            await asyncio.sleep(5)
        finally:
            print("bg finally on shutdown")
    asyncio.create_task(bg())
    await asyncio.sleep(0)
    print("main returns")
    return 7

print(asyncio.run(main()))
