import asyncio, time
# Timers that are due together fire in the same loop pass, in deadline order, and a task's sleep(0)
# puts its next step after all of them. Each group's tasks start their sleeps, then the loop is held
# up past every deadline (time.sleep blocks it), so all the timers are due when it next looks, on any
# machine. Without the hold, sleeps started microseconds apart can fall into two passes, in CPython too.
async def w(i, d):
    await asyncio.sleep(d)
    print(i, "a")
    await asyncio.sleep(0)
    print(i, "b")
async def main():
    g = asyncio.gather(*(w(i, 0.01) for i in range(5)))
    await asyncio.sleep(0)
    time.sleep(0.03)
    await g
    ts = [asyncio.create_task(w(i, 0.02)) for i in range(5, 10)]
    await asyncio.sleep(0)
    time.sleep(0.04)
    for t in ts:
        await t
    async with asyncio.TaskGroup() as tg:
        for i in range(10, 14):
            tg.create_task(w(i, 0.005))
        await asyncio.sleep(0)
        time.sleep(0.02)
asyncio.run(main())
