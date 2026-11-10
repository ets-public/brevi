# A minimal MCP server for the Windows checks: Brevi's own mcp over streamable HTTP
# (mcp.run(transport="streamable-http")), driven by tests/windows/mcp_check.py with raw HTTP POSTs.
import sys

from mcp.server.mcpserver import MCPServer

mcp = MCPServer("win-http")


@mcp.tool()
def add(a: int, b: int = 2) -> int:
    """Add two numbers."""
    return a + b


if __name__ == "__main__":
    mcp.run(transport="streamable-http", port=int(sys.argv[1]) if len(sys.argv) > 1 else 8765)
