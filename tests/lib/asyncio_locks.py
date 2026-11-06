import asyncio

async def locked_worker(lock, name, log):
    async with lock:
        log.append(name + "+")
        await asyncio.sleep(0.001)
        log.append(name + "-")

async def waiter(ev, name):
    print(name, "waiting")
    await ev.wait()
    print(name, "released")

async def sem_worker(sem, i, state):
    async with sem:
        state["now"] += 1
        state["max"] = max(state["max"], state["now"])
        await asyncio.sleep(0.002)
        state["now"] -= 1
    return i

async def main():
    lock = asyncio.Lock()
    log = []
    await asyncio.gather(*(locked_worker(lock, n, log) for n in "abc"))
    print(log, lock.locked())
    ev = asyncio.Event()
    ts = [asyncio.create_task(waiter(ev, f"w{i}")) for i in range(3)]
    await asyncio.sleep(0)
    print("set", ev.is_set())
    ev.set()
    await asyncio.gather(*ts)
    print(ev.is_set(), await ev.wait())
    ev.clear()
    print(ev.is_set())
    sem = asyncio.Semaphore(2)
    state = {"now": 0, "max": 0}
    print(await asyncio.gather(*(sem_worker(sem, i, state) for i in range(5))), state)
    bs = asyncio.BoundedSemaphore(1)
    await bs.acquire()
    bs.release()
    try:
        bs.release()
    except ValueError as e:
        print(e)
    cond = asyncio.Condition()
    items = []
    async def consume(n):
        async with cond:
            await cond.wait_for(lambda: len(items) >= n)
            print("consumer", n, "sees", list(items))
    cs = [asyncio.create_task(consume(n)) for n in (1, 2)]
    for i in range(2):
        await asyncio.sleep(0)
        async with cond:
            items.append(i)
            cond.notify_all()
    await asyncio.gather(*cs)
    print(lock.locked(), await lock.acquire(), lock.locked())
    lock.release()
    try:
        lock.release()
    except RuntimeError as e:
        print(e)

asyncio.run(main())
