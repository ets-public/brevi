import asyncio, sys
async def ag():
    try:
        yield 1
        yield 2
    finally:
        print("fin")
async def main():
    g = ag()
    a = g.__anext__()
    print(type(a).__name__, asyncio.iscoroutine(a), repr(a)[:30])
    print(await a)
    try:
        await a
    except Exception as e: print(type(e).__name__, e)
    print(await g.aclose())
    try: print(await g.__anext__())
    except StopAsyncIteration as e: print("SAI", repr(e))
    try: print(await g.athrow(ValueError))
    except BaseException as e: print("athrow done", type(e).__name__)
    g2 = ag()
    try: print(await g2.athrow(ValueError("x")))
    except BaseException as e: print("athrow fresh", type(e).__name__, e)
    print(await g2.aclose())
    g3 = ag()
    print(await g3.aclose())
    print(type(g3).__name__, g3.__name__, g3.ag_running)
    t = asyncio.create_task(ag().__anext__())
    print(await t)
asyncio.run(main())
