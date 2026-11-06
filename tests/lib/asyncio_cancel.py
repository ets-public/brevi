import asyncio

async def worker(name):
    try:
        print(name, "start")
        await asyncio.sleep(10)
        print(name, "never")
    except asyncio.CancelledError as e:
        print(name, "cancelled", repr(e), e.args)
        raise
    finally:
        print(name, "cleanup")

async def stubborn():
    try:
        await asyncio.sleep(10)
    except asyncio.CancelledError:
        print("stubborn caught it")
        return "kept going"

async def main():
    t = asyncio.create_task(worker("w1"))
    await asyncio.sleep(0.01)
    print("cancel ->", t.cancel())
    try:
        await t
    except asyncio.CancelledError:
        print("main saw cancel", t.cancelled(), t.done())
    t2 = asyncio.create_task(worker("w2"))
    await asyncio.sleep(0)
    t2.cancel("because")
    try:
        await t2
    except asyncio.CancelledError as e:
        print("msg", e.args)
    t3 = asyncio.create_task(stubborn())
    await asyncio.sleep(0)
    t3.cancel()
    print(await t3, t3.cancelled())
    t4 = asyncio.create_task(worker("w4"))
    print("cancel before start", t4.cancel())
    try:
        await t4
    except asyncio.CancelledError:
        print("w4 cancelled", t4.cancelled())
    print(t.cancel(), t.cancelling())

asyncio.run(main())
