# Change events (2026-07-28 subscriptions/listen; resources/updated for handshake clients) published from tools.
# Each tool waits a moment before it answers, so its events are on the wire before its reply in any runtime.
import asyncio
from mcp.server.mcpserver import MCPServer, Context

mcp = MCPServer("subs")
notes = {"a": "first"}


@mcp.resource("note://{name}")
def note(name: str) -> str:
    """A note."""
    return notes.get(name, "")


@mcp.tool()
async def edit(name: str, text: str, ctx: Context) -> str:
    """Change a note and tell subscribers."""
    notes[name] = text
    await ctx.notify_resource_updated("note://" + name)
    await asyncio.sleep(0.05)
    return "edited " + name


@mcp.tool()
async def grow(ctx: Context) -> str:
    """Say the tool, prompt and resource lists changed."""
    await ctx.notify_tools_changed()
    await ctx.notify_prompts_changed()
    await ctx.notify_resources_changed()
    await asyncio.sleep(0.05)
    return "grown"


@mcp.tool()
async def legacy_edit(name: str, text: str, ctx: Context) -> str:
    """Change a note and send resources/updated on the session (handshake clients)."""
    notes[name] = text
    await ctx.session.send_resource_updated("note://" + name)
    await asyncio.sleep(0.05)
    return "sent " + name


@mcp.tool()
async def burst(ctx: Context) -> str:
    """Publish five events for one URI in a row."""
    for i in range(5):
        await ctx.notify_resource_updated("note://a")
    await asyncio.sleep(0.05)
    return "burst"


mcp.run()
