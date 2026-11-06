import asyncio

async def producer(q, n):
    for i in range(n):
        await q.put(i)
        print("put", i, q.qsize())
    await q.put(None)

async def consumer(q, name):
    total = 0
    while True:
        item = await q.get()
        if item is None:
            q.task_done()
            break
        print(name, "got", item)
        total += item
        q.task_done()
        await asyncio.sleep(0)
    return total

async def main():
    q = asyncio.Queue(maxsize=2)
    p = asyncio.create_task(producer(q, 6))
    c = asyncio.create_task(consumer(q, "c1"))
    print(await asyncio.gather(p, c))
    print(q.empty(), q.full(), q.qsize(), q.maxsize)
    pq = asyncio.PriorityQueue()
    for x in [5, 1, 4, 2]:
        pq.put_nowait((x, str(x)))
    print([pq.get_nowait() for _ in range(4)])
    lq = asyncio.LifoQueue()
    for x in "abc":
        await lq.put(x)
    print([await lq.get() for _ in range(3)])
    try:
        lq.get_nowait()
    except asyncio.QueueEmpty:
        print("empty")
    q2 = asyncio.Queue(1)
    q2.put_nowait(1)
    try:
        q2.put_nowait(2)
    except asyncio.QueueFull:
        print("full")
    q3 = asyncio.Queue()
    for i in range(3):
        q3.put_nowait(i)
    async def drain():
        while True:
            x = await q3.get()
            print("drain", x)
            q3.task_done()
    d = asyncio.create_task(drain())
    await q3.join()
    print("joined")
    d.cancel()
    print(str(q3))

asyncio.run(main())
