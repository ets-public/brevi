import asyncio

async def hello(name, delay):
    print(f"hello {name}")
    await asyncio.sleep(delay)
    print(f"bye {name}")
    return len(name)

async def ticker(n):
    for i in range(n):
        print("tick", i)
        await asyncio.sleep(0)
    return "ticked"

async def main():
    r = await hello("world", 0.01)
    print("result", r)
    t1 = asyncio.create_task(hello("one", 0.03))
    t2 = asyncio.create_task(hello("three", 0.01))
    t3 = asyncio.create_task(ticker(3))
    print("tasks made", t1.done(), t2.get_name(), t3.get_name())
    print(await t1, await t2, await t3)
    print(t1.done(), t1.result(), t1.cancelled())
    res = await asyncio.gather(ticker(2), hello("g", 0), ticker(1))
    print(res)
    print(await asyncio.sleep(0.001, result="slept"))
    return "main done"

print(asyncio.run(main()))
print(asyncio.run(ticker(2)))
