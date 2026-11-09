import asyncio

xs = {3, -1, 5}


async def f():
    await asyncio.sleep(0)
    return 7


async def main():
    print(xs)
    print(await f())


asyncio.run(main())
