# The low-level Server's change events: resources/subscribe and resources/unsubscribe handlers with
# session.send_resource_updated (handshake era), and a ListenHandler over an InMemorySubscriptionBus that a
# tool publishes to (2026-07-28 subscriptions/listen).
import asyncio
import mcp.types as types
from mcp.server.lowlevel import Server, NotificationOptions
from mcp.server.stdio import stdio_server
from mcp.server.subscriptions import InMemorySubscriptionBus, ListenHandler, ResourceUpdated, ToolsListChanged

bus = InMemorySubscriptionBus()
watched = set()
seen = []


def on_event(event):
    seen.append(type(event).__name__)


stop = bus.subscribe(on_event)


async def subscribe(ctx, params):
    watched.add(str(params.uri))
    return types.EmptyResult()


async def unsubscribe(ctx, params):
    watched.discard(str(params.uri))
    return types.EmptyResult()


async def list_tools(ctx, params):
    return types.ListToolsResult(tools=[types.Tool(name="touch", input_schema={"type": "object", "properties": {"uri": {"type": "string"}}})])


async def call_tool(ctx, params):
    uri = (params.arguments or {}).get("uri", "")
    if params.name == "touch":
        if uri in watched:
            await ctx.session.send_resource_updated(uri)
        await bus.publish(ResourceUpdated(uri=uri))
        await bus.publish(ToolsListChanged())
        await ctx.session.send_resource_list_changed()
        await asyncio.sleep(0.05)
        return types.CallToolResult(content=[types.TextContent(type="text", text="watched=%s seen=%s" % (sorted(watched), seen))])
    if params.name == "stop":
        stop()
        stop()
        await bus.publish(ToolsListChanged())
        await asyncio.sleep(0.05)
        return types.CallToolResult(content=[types.TextContent(type="text", text="seen=%s" % seen)])
    raise ValueError("unknown " + params.name)


server = Server("low-subs", version="1", on_list_tools=list_tools, on_call_tool=call_tool, on_subscribe_resource=subscribe,
                on_unsubscribe_resource=unsubscribe, on_subscriptions_listen=ListenHandler(bus, max_subscriptions=1))


async def main():
    async with stdio_server() as (r, w):
        await server.run(r, w, server.create_initialization_options(notification_options=NotificationOptions(resources_changed=True)))


asyncio.run(main())
print("after run")
