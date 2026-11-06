import asyncio
async def work(x):
    await asyncio.sleep(0.001)
    return x * 2
loop = asyncio.new_event_loop()
print(loop.run_until_complete(work(4)))
print(loop.run_until_complete(asyncio.gather(work(1), work(2))))
print(loop.is_running(), loop.is_closed())
loop.close()
print(loop.is_closed())
try:
    asyncio.get_running_loop()
except RuntimeError as e:
    print(e)
async def nested():
    try:
        asyncio.run(work(1))
    except RuntimeError as e:
        print(e)
    c = work(1)
    c.close()
asyncio.run(nested())
try:
    asyncio.run(5)
except ValueError as e:
    print(e)
r = asyncio.Runner()
with asyncio.Runner() as runner:
    print(runner.run(work(5)), runner.run(work(6)))
