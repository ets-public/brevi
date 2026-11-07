from mcp.server.mcpserver import MCPServer

mcp = MCPServer("demo")

@mcp.tool()
def add(a: int, b: int = 2) -> int:
    """Add two numbers."""
    return a + b

@mcp.tool()
async def greet(name: str, excited: bool = False) -> str:
    return f"Hello {name}" + ("!" if excited else ".")

@mcp.resource("greeting://{name}")
def greeting(name: str) -> str:
    """A greeting."""
    return f"Hi {name}"

@mcp.prompt()
def review(code: str) -> str:
    return f"Review this:\n{code}"

if __name__ == "__main__":
    mcp.run()
