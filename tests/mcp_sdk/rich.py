from typing import Annotated, Literal, Optional
from contextlib import asynccontextmanager
from pydantic import BaseModel, Field
from mcp.server.mcpserver import MCPServer, Context, Image
from mcp.types import TextContent, ToolAnnotations, Completion, PromptReference, ResourceTemplateReference
from mcp.server.mcpserver.prompts.base import UserMessage, AssistantMessage, Message
from mcp.shared.exceptions import MCPError
from mcp.server.mcpserver.exceptions import ToolError


class Point(BaseModel):
    x: int
    y: int = 0
    label: Optional[str] = None


class Weather(BaseModel):
    """Weather report."""
    city: str
    temp: float = Field(description="Celsius")
    tags: list[str] = []


@asynccontextmanager
async def life(server):
    yield {"db": "conn-1"}


mcp = MCPServer("demo3", instructions="Use tools", version="0.3", lifespan=life, website_url="https://example.com")


@mcp.tool(title="Move a point", annotations=ToolAnnotations(readOnlyHint=True))
def move(p: Point, dx: int = 1, mode: Literal["fast", "slow"] = "fast") -> Point:
    """Move p by dx."""
    return Point(x=p.x + dx, y=p.y, label=mode)


@mcp.tool()
def weather(city: Annotated[str, Field(description="City name", min_length=2)], days: Annotated[int, Field(ge=1, le=7)] = 1) -> Weather:
    return Weather(city=city, temp=21.5, tags=["sunny"] * days)


@mcp.tool()
def maybe(n: Optional[int] = None, s: str | None = None) -> Optional[str]:
    if n is None:
        return None
    return str(n) + (s or "")


@mcp.tool()
def many(items: list[Point]) -> list[Point]:
    return list(reversed(items))


@mcp.tool()
def stats(xs: list[float]) -> dict[str, float]:
    return {"sum": sum(xs), "n": float(len(xs))}


@mcp.tool(structured_output=False)
def raw(x: int) -> int:
    return x * 2


@mcp.tool()
def fails(kind: str) -> str:
    if kind == "tool":
        raise ToolError("nope " + kind)
    if kind == "mcp":
        raise MCPError(code=-32099, message="custom", data={"k": 1})
    if kind == "key":
        raise KeyError("missing")
    return kind


@mcp.tool()
def pic() -> Image:
    return Image(data=b"\x89PNG\r\n", format="png")


@mcp.tool()
def mixed() -> list:
    return ["a", 1, {"b": 2}]


@mcp.tool()
async def lifespan_info(ctx: Context) -> str:
    return ctx.request_context.lifespan_context["db"] + " " + ctx.request_id


@mcp.tool()
async def reader(uri: str, ctx: Context) -> str:
    out = await ctx.read_resource(uri)
    return "|".join(c.content if isinstance(c.content, str) else "bytes" for c in out)


@mcp.tool(name="renamed", description="Custom description")
def orig(a: bool, b: bytes = b"x") -> bool:
    return not a


@mcp.resource("config://app", mime_type="application/json")
def app_config() -> dict:
    return {"debug": True, "level": 3}


@mcp.resource("data://blob")
def blob() -> bytes:
    return b"\x00\x01\x02"


@mcp.resource("users://{uid}/profile/{field}")
async def profile(uid: str, field: str) -> str:
    return f"{uid}:{field}"


@mcp.resource("files://{+path}")
def files(path: str) -> str:
    return "file " + path


@mcp.resource("search://items{?q,limit}")
def search(q: str = "", limit: int = 10) -> str:
    return f"q={q} limit={limit}"


@mcp.resource("err://{x}")
def err(x: str) -> str:
    raise ValueError("bad resource " + x)


@mcp.prompt(title="Chat")
def chat(topic: str, tone: str = "friendly") -> list[Message]:
    """Start a chat."""
    return [UserMessage(f"Talk about {topic}"), AssistantMessage(f"Sure, in a {tone} tone."), {"role": "user", "content": "go"}]


@mcp.prompt()
async def failing(x: str) -> str:
    raise RuntimeError("boom " + x)


@mcp.completion()
async def complete(ref, argument, context):
    if isinstance(ref, PromptReference) and argument.name == "topic":
        return Completion(values=[v for v in ["python", "rust", "go"] if v.startswith(argument.value)], total=3, has_more=False)
    if isinstance(ref, ResourceTemplateReference):
        return Completion(values=["u1", "u2"])
    return None


if __name__ == "__main__":
    mcp.run()
