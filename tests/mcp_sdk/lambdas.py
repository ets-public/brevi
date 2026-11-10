# Lambdas handed to the SDK: add_tool(lambda ...), the tool decorator called on a lambda, a lambda added from
# inside a tool, and a lambda without name= (refused, as CPython's mcp refuses it).
from mcp.server.mcpserver import MCPServer, Context
from mcp.server.mcpserver.prompts import Prompt

mcp = MCPServer("lambdas")
k = 10
mcp.add_tool(lambda a, b=2: a * b + k, name="mul", description="Multiply, plus k.")
sq = mcp.tool(name="sq", description="Square.")(lambda x: x * x)
mcp.add_prompt(Prompt.from_function(lambda topic: "Tell me about " + topic, name="about", description="A prompt."))
try:
    mcp.add_tool(lambda: 1)
except ValueError as e:
    print("refused:", e)


@mcp.tool()
async def grow(n: int, ctx: Context) -> str:
    """Add a tool made from a lambda over n."""
    mcp.add_tool(lambda s: s * n, name="rep%d" % n, description="Repeat.")
    await ctx.notify_tools_changed()
    return "added rep%d" % n


mcp.run()
