import asyncio
from asyncio import sleep, gather, Queue

class Account:
    def __init__(self, bal):
        self.bal = bal
        self.lock = asyncio.Lock()
    async def withdraw(self, n):
        async with self.lock:
            b = self.bal
            await sleep(0)
            if b < n:
                raise ValueError(f"insufficient: {b} < {n}")
            self.bal = b - n
            return self.bal
    @staticmethod
    async def make(b):
        await sleep(0)
        return Account(b)
    @classmethod
    async def zero(cls):
        return cls(0)

async def fib(n):
    if n < 2:
        return n
    return await fib(n - 1) + await fib(n - 2)

def make_adder(k):
    async def add(x):
        await sleep(0)
        return x + k
    return add

async def main():
    acc = await Account.make(100)
    rs = await gather(*(acc.withdraw(30) for _ in range(4)), return_exceptions=True)
    print([r if isinstance(r, int) else str(r) for r in rs], acc.bal)
    print((await Account.zero()).bal)
    print(await fib(15))
    add5 = make_adder(5)
    print([await add5(i) for i in range(3)], {i: await add5(i) for i in range(2)})
    print(f"value {await add5(10)}", max(await add5(1), await add5(2)))
    coros = [add5(i) for i in range(3)]
    print(await gather(*coros))
    c = add5(1)
    print(await c)
    try:
        await c
    except RuntimeError as e:
        print("reuse:", e)
    async def raiser():
        await sleep(0)
        raise KeyError("k")
    try:
        await raiser()
    except KeyError as e:
        print("keyerror", e)
    try:
        try:
            await raiser()
        except KeyError as e:
            raise RuntimeError("wrapped") from e
    except RuntimeError as e:
        print(e)
    t = asyncio.create_task(raiser())
    await sleep(0.001)
    print(t.done(), repr(t.exception()))
    print(asyncio.iscoroutine(c), asyncio.iscoroutine(5))
    q = Queue()
    await q.put(1)
    print(q.get_nowait())
    print(type(c).__name__, c.__name__ if hasattr(c, "__name__") else None)

asyncio.run(main())
print(asyncio.run(fib(10)))
