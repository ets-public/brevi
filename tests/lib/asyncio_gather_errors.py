import asyncio

async def ok(x, d):
    await asyncio.sleep(d)
    print("ok", x)
    return x

async def bad(x, d):
    await asyncio.sleep(d)
    print("bad", x)
    raise ValueError(f"bad {x}")

async def main():
    r = await asyncio.gather(ok(1, 0.02), bad(2, 0.01), ok(3, 0.03), return_exceptions=True)
    print([type(v).__name__ for v in r], r[0], r[2], str(r[1]))
    try:
        await asyncio.gather(ok(4, 0.02), bad(5, 0.01))
    except ValueError as e:
        print("caught", e)
    await asyncio.sleep(0.03)
    print(await asyncio.gather())
    f = asyncio.gather(ok(6, 0.05), ok(7, 0.05))
    await asyncio.sleep(0.01)
    print("cancel gather", f.cancel())
    try:
        await f
    except asyncio.CancelledError:
        print("gather cancelled", f.cancelled())

asyncio.run(main())
