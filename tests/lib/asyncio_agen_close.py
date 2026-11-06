import asyncio
async def slow():
    yield 1
async def main():
    g = slow(); c = g.aclose(); c.close()
    try: print("A", await g.athrow(KeyError))
    except BaseException as e: print("A", type(e).__name__)
    g = slow(); a = anext(g); a.close()
    try: print("B", await anext(g))
    except BaseException as e: print("B", type(e).__name__)
    g = slow(); c = g.athrow(ValueError); c.close()
    try: print("D", await anext(g))
    except BaseException as e: print("D", type(e).__name__)
    g = slow(); print(await anext(g)); c = g.aclose(); c.close()
    try: print("E", await anext(g))
    except BaseException as e: print("E", type(e).__name__)
async def slow2():
    try:
        yield 1
        yield 2
    finally:
        print("fin")
async def main2():
    g = slow2(); print(await anext(g)); c = anext(g); c.close(); print("closed asend")
    try: print("E", await anext(g))
    except BaseException as e: print("E", type(e).__name__)
    print("end")


asyncio.run(main2())
asyncio.run(main())
