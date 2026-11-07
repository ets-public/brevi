from mcp.server.mcpserver import MCPServer

mcp = MCPServer("web")


@mcp.tool()
def hi() -> str:
    return "hi"


mcp.run(transport="streamable-http")
