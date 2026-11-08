# The official mcp client (streamable HTTP and SSE) against a server: python3 client.py streamable|sse PORT -- COMMAND...
import asyncio
import socket
import subprocess
import sys
import time

from mcp import ClientSession
from mcp.client.sse import sse_client
from mcp.client.streamable_http import streamable_http_client

kind, port = sys.argv[1], int(sys.argv[2])
cmd = sys.argv[sys.argv.index("--") + 1:]


async def session(r, w):
    seen = []

    async def progress(p, total, message):
        seen.append((p, total))

    async with ClientSession(r, w) as s:
        info = await s.initialize()
        tools = await s.list_tools()
        add = await s.call_tool("add", {"a": 20, "b": 22})
        slow = await s.call_tool("slow", {"n": 3}, progress_callback=progress)
        return [info.server_info.name, sorted(t.name for t in tools.tools), add.structured_content, slow.content[0].text, seen]


async def main():
    if kind == "sse":
        async with sse_client("http://127.0.0.1:%d/sse" % port) as (r, w):
            return await session(r, w)
    async with streamable_http_client("http://127.0.0.1:%d/mcp" % port) as (r, w, *_):
        return await session(r, w)


p = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    t0 = time.time()
    while True:
        try:
            socket.create_connection(("127.0.0.1", port), timeout=1).close()
            break
        except OSError:
            if p.poll() is not None or time.time() - t0 > 60:
                raise SystemExit("server did not start")
            time.sleep(0.1)
    print(asyncio.run(asyncio.wait_for(main(), 60)))
finally:
    p.terminate()
    p.wait()
