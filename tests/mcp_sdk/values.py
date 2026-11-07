from mcp.server.mcpserver import MCPServer, Context

mcp = MCPServer("demo2", instructions="Be nice", version="1.2")

@mcp.tool()
def echo(text: str) -> str:
    return text

@mcp.tool()
def f(x: float) -> float:
    return x * 1.5

@mcp.tool()
def nothing() -> None:
    return None

@mcp.tool()
def lst(n: int) -> list[int]:
    return list(range(n))

@mcp.tool()
def dct(k: str) -> dict:
    return {k: 1.0, "u": "é"}

@mcp.tool()
def plain(x):
    return {"x": x}

@mcp.tool()
def boom(x: int) -> int:
    raise ValueError("bad " + str(x))

@mcp.tool()
async def logs(n: int, ctx: Context) -> str:
    await ctx.info("hello")
    await ctx.debug("dbg")
    await ctx.warning("w")
    await ctx.report_progress(1, 2, "half")
    return "done"

if __name__ == "__main__":
    mcp.run()
