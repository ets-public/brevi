# An MCP server on the official SDK served over HTTP: python3 server.py TRANSPORT [json_response=1] [stateless_http=1] PORT
import asyncio
import sys

from mcp.server.mcpserver import Context, MCPServer

mcp = MCPServer("http-demo")


@mcp.tool()
def add(a: int, b: int) -> int:
    """Add two numbers."""
    return a + b


@mcp.tool()
async def slow(n: int, ctx: Context) -> str:
    """Report progress."""
    for i in range(n):
        await ctx.report_progress(i + 1, n)
        await ctx.info("step %d" % (i + 1))
    return "done %d" % n


@mcp.tool()
async def touch(uri: str, ctx: Context) -> str:
    """Publish change events: to subscriptions/listen streams, and resources/updated on the session."""
    await ctx.notify_resource_updated(uri)
    await ctx.notify_tools_changed()
    if ctx.protocol_version != "2026-07-28":
        await ctx.session.send_resource_updated(uri)
    await asyncio.sleep(0.05)
    return "touched " + uri


kind = sys.argv[1] if len(sys.argv) > 1 else "streamable-http"
port = 8000
extra = {}
for a in sys.argv[2:]:
    if "=" in a:
        k, v = a.split("=")
        extra[k] = v == "1"
    else:
        port = int(a)
mcp.run(transport=kind, port=port, **extra)
