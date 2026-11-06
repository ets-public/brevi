import asyncio

async def bad_stop():
    yield 1
    raise StopAsyncIteration

async def bad_stop2():
    yield 1
    raise StopIteration

async def raises():
    yield 1
    raise KeyError("k")

async def stubborn():
    try:
        yield 1
    except GeneratorExit:
        yield 2

async def slow_close():
    try:
        yield 1
    finally:
        print("closing slowly")
        await asyncio.sleep(0.001)
        print("closed")

async def slow():
    for i in range(3):
        await asyncio.sleep(0.002)
        yield i

async def show(tag, aw):
    try:
        print(tag, await aw)
    except BaseException as e:
        print(tag, type(e).__name__, e)

async def main():
    g = bad_stop()
    await show("bs1", anext(g))
    await show("bs2", anext(g))
    await show("bs3", anext(g))
    g = bad_stop2()
    await show("bi1", anext(g))
    await show("bi2", anext(g))
    g = raises()
    await show("r1", anext(g))
    await show("r2", anext(g))
    await show("r3", anext(g))
    g = stubborn()
    await show("s1", anext(g))
    await show("s2", g.aclose())
    g = slow_close()
    await show("c1", anext(g))
    await show("c2", g.aclose())
    await show("c3", g.aclose())
    g = slow()
    t1 = asyncio.create_task(anext(g))
    await asyncio.sleep(0)
    await show("race", anext(g))
    print("t1", await t1)
    await show("send-fresh", slow().asend(5))
    g = slow()
    await show("athrow-fresh", g.athrow(ValueError("v")))
    await show("after athrow-fresh", anext(g))
    g = slow()
    a = anext(g)
    await show("a1", a)
    await show("reuse", a)
    c = g.aclose()
    await show("cl", c)
    await show("cl reuse", c)
    g = slow()
    print(type(g).__name__, g.__name__, g.ag_running)
    a, c = anext(g), g.aclose()
    print(type(a).__name__, type(c).__name__, asyncio.iscoroutine(a))
    a.close()
    c.close()
    await show("athrow cls", g.athrow(KeyError))
    async def agen_wait_for():
        async for x in slow():
            yield x
    async def consume():
        return [x async for x in agen_wait_for()]
    await show("wait_for", asyncio.wait_for(consume(), 0.003))
    await show("wait_for ok", asyncio.wait_for(consume(), 1))
    try:
        async with asyncio.timeout(0.003):
            async for x in slow():
                print("slow", x)
    except TimeoutError:
        print("timed out")
    await asyncio.sleep(0.01)
    print("done")

asyncio.run(main())
