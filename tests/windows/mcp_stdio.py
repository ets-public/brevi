# A minimal MCP server for the Windows checks: Brevi's own mcp over stdio (mcp.run(), the default transport),
# driven by tests/windows/mcp_check.py over pipes.
from mcp.server.mcpserver import MCPServer

mcp = MCPServer("win-stdio")


@mcp.tool()
def add(a: int, b: int = 2) -> int:
    """Add two numbers."""
    return a + b


if __name__ == "__main__":
    mcp.run()
