# Many small asyncio tasks, queues and sleeps (the event-loop speed gap).
import asyncio
import sys
import time


async def worker(q, out):
    while True:
        v = await q.get()
        if v is None:
            q.task_done()
            return
        await asyncio.sleep(0)
        out.append(v * 2)
        q.task_done()


async def main(n):
    q = asyncio.Queue()
    out = []
    ws = [asyncio.create_task(worker(q, out)) for _ in range(8)]
    for i in range(n):
        await q.put(i)
    for _ in ws:
        await q.put(None)
    await q.join()
    await asyncio.gather(*ws)

    async def leaf(i):
        await asyncio.sleep(0)
        return i
    s = 0
    for k in range(n // 100):
        s += sum(await asyncio.gather(*[leaf(i) for i in range(100)]))
    return sum(out) + s


n = int(sys.argv[1]) if len(sys.argv) > 1 else 100000
t = time.perf_counter()
print(asyncio.run(main(n)))
print("time %.3f" % (time.perf_counter() - t), file=sys.stderr)
