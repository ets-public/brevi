from mcp.server.mcpserver import MCPServer

mcp = MCPServer("web")


@mcp.tool()
def hi() -> str:
    return "hi"


app = mcp.streamable_http_app()
