import asyncio

async def slow(d, v):
    try:
        await asyncio.sleep(d)
        return v
    finally:
        print("slow", v, "finally")

async def main():
    print(await asyncio.wait_for(slow(0.001, "fast"), timeout=1))
    try:
        await asyncio.wait_for(slow(1, "late"), timeout=0.01)
    except asyncio.TimeoutError:
        print("timed out 1")
    try:
        await asyncio.wait_for(slow(1, "late2"), timeout=0)
    except TimeoutError as e:
        print("timed out 2", type(e).__name__)
    try:
        async with asyncio.timeout(0.01) as cm:
            print("in timeout", cm.expired())
            await asyncio.sleep(1)
    except TimeoutError:
        print("timeout block", cm.expired())
    async with asyncio.timeout(1) as cm:
        await asyncio.sleep(0.001)
    print("not expired", cm.expired())
    async with asyncio.timeout(None) as cm:
        cm.reschedule(asyncio.get_running_loop().time() + 5)
        await asyncio.sleep(0)
    print(cm.expired())
    t = asyncio.create_task(slow(0.01, "shielded"))
    try:
        await asyncio.wait_for(asyncio.shield(t), 0.001)
    except TimeoutError:
        print("shield timeout, task done:", t.done())
    print(await t)
    print(asyncio.TimeoutError is TimeoutError)

asyncio.run(main())
