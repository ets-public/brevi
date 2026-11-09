# An MCP server that logs to stderr with logging, as most do: built with Brevi's own mcp and logging.
import logging
logging.basicConfig(level=logging.INFO, format="%(levelname)s %(name)s: %(message)s")
log = logging.getLogger("demo")
from mcp.server.mcpserver import MCPServer

mcp = MCPServer("demo")


@mcp.tool()
def add(a: int, b: int = 2) -> int:
    """Add two numbers."""
    log.info("add %s %s", a, b)
    return a + b


@mcp.tool()
async def lookup(key: str) -> str:
    """Look a key up."""
    try:
        return {"a": "apple"}[key]
    except KeyError:
        log.exception("no key %r", key)
        raise ValueError(f"unknown key {key}")


if __name__ == "__main__":
    log.debug("hidden")
    mcp.run()
