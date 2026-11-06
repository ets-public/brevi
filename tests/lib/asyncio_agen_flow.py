import asyncio

async def ticker(n, delay):
    try:
        for i in range(n):
            await asyncio.sleep(delay)
            yield i
    finally:
        print("ticker done", n)

async def squares(src):
    async for x in src:
        yield x * x

class Countdown:
    def __init__(self, n):
        self.n = n
    def __aiter__(self):
        return self.gen()
    async def gen(self):
        while self.n > 0:
            yield self.n
            self.n -= 1
            await asyncio.sleep(0)

async def echo():
    got = None
    while True:
        try:
            got = yield got
        except ValueError as e:
            print("echo caught", e)
            got = "after error"
        if got == "stop":
            return

async def main():
    async for v in ticker(3, 0.001):
        print("tick", v)
    async for v in squares(ticker(4, 0)):
        print("sq", v)
    print([x async for x in ticker(3, 0)])
    print({x: x * 2 async for x in Countdown(3)})
    print({x % 2 async for x in Countdown(5)})
    print([x async for x in Countdown(6) if x % 2 if x > 1])
    print([(x, y) async for x in Countdown(2) for y in range(2)])
    async def double(v):
        await asyncio.sleep(0)
        return v * 2
    print([await double(i) for i in range(4)])
    print([await double(x) async for x in Countdown(3)])
    async for v in ticker(10, 0):
        if v == 2:
            print("breaking")
            break
        print("got", v)
    print("after break")
    await asyncio.sleep(0)
    print("after sleep 1")
    await asyncio.sleep(0)
    print("after sleep 2")
    e = echo()
    print(await e.asend(None))
    print(await e.asend(1))
    print(await e.athrow(ValueError("boom")))
    print(await e.asend(2))
    try:
        await e.asend("stop")
    except StopAsyncIteration:
        print("echo finished")
    g = ticker(5, 0)
    print(await anext(g), await anext(g))
    print(await g.aclose())
    try:
        await anext(g)
    except StopAsyncIteration:
        print("closed gen stops")
    left_open = ticker(5, 0)
    print(await anext(left_open))
    print("main ends")
    return left_open

keep = asyncio.run(main())
print("run returned")
