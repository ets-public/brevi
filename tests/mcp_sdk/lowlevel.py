import asyncio
import mcp.types as types
from mcp.server.lowlevel import Server, NotificationOptions
from mcp.server.stdio import stdio_server


async def list_tools(ctx, params):
    return types.ListToolsResult(tools=[types.Tool(name="sq", description="Square", input_schema={"type": "object", "properties": {"n": {"type": "number"}}, "required": ["n"]})])


async def call_tool(ctx, params):
    if params.name == "sq":
        n = (params.arguments or {}).get("n", 0)
        return types.CallToolResult(content=[types.TextContent(type="text", text=str(n * n))])
    if params.name == "slow":
        await asyncio.sleep(0.3)
        return types.CallToolResult(content=[types.TextContent(type="text", text="slow done")])
    if params.name == "dict":
        return {"content": [{"type": "text", "text": "from dict"}], "extra": 1}
    if params.name == "bad":
        return {"content": "nope"}
    if params.name == "err":
        return types.ErrorData(code=-32000, message="as data")
    raise ValueError("unknown " + params.name)


async def list_prompts(ctx, params):
    return types.ListPromptsResult(prompts=[types.Prompt(name="p1", arguments=[types.PromptArgument(name="a", required=True)])])


async def get_prompt(ctx, params):
    return types.GetPromptResult(messages=[types.PromptMessage(role="user", content=types.TextContent(type="text", text="hi " + str(params.arguments)))])


server = Server("low", version="9", on_list_tools=list_tools, on_call_tool=call_tool, on_list_prompts=list_prompts, on_get_prompt=get_prompt)


async def main():
    async with stdio_server() as (r, w):
        await server.run(r, w, server.create_initialization_options(notification_options=NotificationOptions(tools_changed=True)))


asyncio.run(main())
print("after run")
