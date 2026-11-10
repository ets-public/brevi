# as_completed yields in completion order. When every awaitable completes in the same loop
# iteration (equal sleep delays), the order is the order the tasks were first scheduled;
# Brevi schedules them in the order they were supplied, so the reviewed program prints
# 1, 2 (its error), 3.
import asyncio


async def job(i, fail=False, delay=0):
    await asyncio.sleep(delay)
    if fail:
        raise ValueError(f"boom-{i}")
    return i


async def main():
    for c in asyncio.as_completed([job(1), job(2, True), job(3)]):
        try:
            print("ok", await c)
        except ValueError as e:
            print("err", e)

    # Different delays: completion order, independent of scheduling order.
    for c in asyncio.as_completed([job(4, delay=0.03), job(5, True, 0.01), job(6, delay=0.02)]):
        try:
            print("ok", await c)
        except ValueError as e:
            print("err", e)

    # The same awaitable given twice is yielded once.
    j = job(7)
    got = []
    async for c in asyncio.as_completed([j, j]):
        got.append(await c)
    print(got)

    # Already-created tasks complete in their own order.
    t8 = asyncio.create_task(job(8, delay=0.02))
    t9 = asyncio.create_task(job(9, delay=0.01))
    async for c in asyncio.as_completed([t8, t9]):
        print("task", await c)

    # The plain (synchronous) iterator form.
    for c in asyncio.as_completed([job(10, delay=0.02), job(11, delay=0.01)]):
        print("plain", await c)


asyncio.run(main())
