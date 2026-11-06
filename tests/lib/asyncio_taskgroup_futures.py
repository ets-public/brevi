import asyncio

async def step(name, d):
    await asyncio.sleep(d)
    print("done", name)
    return name.upper()

async def main():
    async with asyncio.TaskGroup() as tg:
        a = tg.create_task(step("a", 0.02))
        b = tg.create_task(step("b", 0.01))
        print("created")
    print(a.result(), b.result())
    cur = asyncio.current_task()
    print(cur.get_name(), len(asyncio.all_tasks()))
    loop = asyncio.get_running_loop()
    fut = loop.create_future()
    loop.call_later(0.01, fut.set_result, 42)
    loop.call_soon(print, "soon", 1)
    print(await fut, fut.done())
    fut2 = loop.create_future()
    fut2.add_done_callback(lambda f: print("callback", f.result()))
    fut2.set_result("x")
    await asyncio.sleep(0)
    print(repr(fut2))
    f3 = asyncio.Future()
    f3.set_exception(ValueError("nope"))
    try:
        await f3
    except ValueError as e:
        print("future raised", e)
    try:
        f3.set_result(1)
    except asyncio.InvalidStateError:
        print("invalid state")

asyncio.run(main())
