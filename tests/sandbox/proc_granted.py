# With --allow native, Brevi's asyncio starts child processes under --sandbox (the glue checks the grant).
import asyncio
async def main():
    p = await asyncio.create_subprocess_exec("/bin/echo", "hi", stdout=asyncio.subprocess.PIPE)
    out, _ = await p.communicate()
    print(out, p.returncode)
asyncio.run(main())
