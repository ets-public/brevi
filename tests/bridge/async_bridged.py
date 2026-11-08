# Async code next to a library CPython runs (brlib, shlex): the library's values and errors are used between
# awaits, while no coroutine crosses into it, so the dynamic build keeps the program native.
import asyncio
import shlex
from brlib import Store, NotFound, apply, describe


def score(x, bonus=0):
    return x * 2 + bonus


class Jobs:
    def __init__(self):
        self.store = Store()

    async def put(self, x):
        await asyncio.sleep(0)
        self.store.add(x)
        return len(self.store)

    async def get(self, i):
        await asyncio.sleep(0.001 * i)
        try:
            return self.store.get(i)
        except NotFound as e:
            return f"missing {i}: {e} ({e.code})"


async def main():
    j = Jobs()
    sizes = await asyncio.gather(*(j.put(x) for x in [5, 7, 9]))
    print(sizes, list(j.store))
    print(await asyncio.gather(j.get(0), j.get(2), j.get(5)))
    print(apply(score, list(j.store), bonus=1), describe(await j.get(1)))
    async with asyncio.timeout(1):
        words = shlex.split('run "two words" --x=1')
    print(words, shlex.join(words))


asyncio.run(main())
