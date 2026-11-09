# Child processes with Brevi's asyncio: create_subprocess_exec and _shell, pipes, communicate, streams,
# signals, env and cwd, errors, and a child that fills its pipe while the program is still writing.
import asyncio
import sys


async def run(*cmd, input=None):
    p = await asyncio.create_subprocess_exec(*cmd, stdin=asyncio.subprocess.PIPE if input else None,
                                             stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
    out, err = await p.communicate(input)
    return p.returncode, out, err


async def main():
    print(await run("echo", "hello", "world"))
    print(await run("sh", "-c", "echo out; echo err >&2; exit 3"))
    print(await run("tr", "a-z", "A-Z", input=b"shout this\n"))
    p = await asyncio.create_subprocess_shell("printf 'a\\nb\\nc'", stdout=asyncio.subprocess.PIPE)
    lines = [l async for l in p.stdout]
    print(lines, await p.wait(), p.returncode)
    p = await asyncio.create_subprocess_exec("sh", "-c", "echo one; sleep 0.05; echo two", stdout=asyncio.subprocess.PIPE)
    print(await p.stdout.readline(), await p.stdout.readline(), await p.stdout.read(), await p.wait())
    p = await asyncio.create_subprocess_exec("sleep", "5")
    p.kill()
    print(await p.wait())
    p = await asyncio.create_subprocess_exec("sh", "-c", "echo x 1>&2", stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.STDOUT)
    print(await p.communicate())
    try:
        await asyncio.create_subprocess_exec("/no/such/prog")
    except FileNotFoundError as e:
        print("FNF", e)
    r = await asyncio.gather(*(run("sh", "-c", f"sleep 0.0{i}; echo {i}") for i in range(4)))
    print(r)
    p = await asyncio.create_subprocess_exec("cat", stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE)
    p.stdin.write(b"x" * 200000)
    await p.stdin.drain()
    p.stdin.close()
    data = await p.stdout.read()
    print(len(data), await p.wait())
    p = await asyncio.create_subprocess_exec("sh", "-c", "exit 0", stdout=asyncio.subprocess.DEVNULL)
    print(await p.wait(), p.stdout)
    try:
        await asyncio.wait_for(run("sleep", "2"), 0.1)
    except TimeoutError:
        print("timeout")

asyncio.run(main())


from asyncio.subprocess import PIPE, STDOUT
import asyncio.subprocess as sp


async def main2():
    p = await asyncio.create_subprocess_exec("sh", "-c", "echo $FOO; pwd", stdout=PIPE, stderr=STDOUT, env={"FOO": "bar", "PATH": "/usr/bin:/bin"}, cwd="/tmp")
    out, _ = await p.communicate()
    print(out.decode().split(), p.returncode, sp.DEVNULL)
    try:
        await asyncio.create_subprocess_exec("ls", cwd="/no/dir")
    except FileNotFoundError as e:
        print(type(e).__name__, e)
    ps = [await asyncio.create_subprocess_exec("true") for _ in range(50)]
    print(sum([await p.wait() for p in ps]))

asyncio.run(main2())
