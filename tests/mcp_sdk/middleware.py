# Server middleware (MCPServer.middleware and MCPServer(middleware=...)), as mcp-server-time uses it: each
# inbound message passes through the chain outermost first; a middleware can refuse a call with an MCPError,
# rewrite the context, or wrap the result. Also MCPServer(subscriptions=False) and a model whose field names
# a class defined later (model_rebuild).
from typing import Any

from mcp.server.context import CallNext, HandlerResult, ServerRequestContext
from mcp.server.mcpserver import MCPServer
from mcp.shared.exceptions import MCPError
from mcp.types import INVALID_PARAMS
from pydantic import BaseModel, Field



class Pair(BaseModel):
    first: "Item"
    later: "Item | None" = Field(default=None, description="the second item, when there is one")


class Item(BaseModel):
    name: str
    size: int


seen = ["model_rebuild " + repr(Pair.model_rebuild())]


async def outer(ctx: ServerRequestContext[Any, Any], call_next: CallNext) -> HandlerResult:
    seen.append("outer " + ctx.method)
    result = await call_next(ctx)
    seen.append("outer done " + ctx.method)
    return result


server = MCPServer("mw", version="1.0", subscriptions=False, middleware=[outer])


async def gate(ctx: ServerRequestContext[Any, Any], call_next: CallNext) -> HandlerResult:
    if ctx.method == "tools/call" and ctx.params is not None:
        name = ctx.params.get("name")
        if isinstance(name, str) and name not in {t.name for t in await server.list_tools()}:
            raise MCPError(code=INVALID_PARAMS, message=f"Unknown tool: {name}")
    return await call_next(ctx)


server.middleware.append(gate)


@server.tool()
def pair(a: str, b: str = "") -> Pair:
    """Two items, the second optional."""
    return Pair(first=Item(name=a, size=len(a)), later=Item(name=b, size=len(b)) if b else None)


@server.tool()
def trail() -> list[str]:
    """What the middleware saw so far."""
    return list(seen)


server.run()
