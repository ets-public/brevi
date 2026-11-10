import asyncio

async def ok(n):
    await asyncio.sleep(0.3 * n)
    print("ok", n)
    return n

async def boom(n, exc):
    await asyncio.sleep(0.005 * n)
    print("boom", n)
    raise exc

async def slow():
    try:
        await asyncio.sleep(1)
    except asyncio.CancelledError:
        print("slow cancelled")
        raise

async def main():
    try:
        async with asyncio.TaskGroup() as tg:
            tg.create_task(ok(1))
            tg.create_task(boom(1, ValueError("v")))
            tg.create_task(slow())
    except* ValueError as e:
        print("caught", repr(e))
    try:
        async with asyncio.TaskGroup() as tg:
            tg.create_task(boom(1, KeyError("a")))
            tg.create_task(boom(1, TypeError("b")))
    except* KeyError as e:
        print("keys", repr(e))
    except* TypeError as e:
        print("types", repr(e))
    try:
        async with asyncio.TaskGroup() as tg:
            tg.create_task(boom(2, ValueError("x")))
            raise RuntimeError("body fails")
    except ExceptionGroup as e:
        print("group", str(e), [repr(x) for x in e.exceptions])
    print("done")

asyncio.run(main())
