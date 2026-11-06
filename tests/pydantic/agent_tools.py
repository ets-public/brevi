# An AI agent's tool calls, checked with pydantic: the arguments a model sends as JSON are validated, and the
# errors go back to the model as text it can fix.
import json
from typing import Literal, Optional
from pydantic import BaseModel, Field, ValidationError, field_validator


class SearchArgs(BaseModel):
    query: str = Field(min_length=1, max_length=200)
    limit: int = Field(default=5, ge=1, le=50)
    sort: Literal["relevance", "date"] = "relevance"
    site: Optional[str] = None

    @field_validator("site")
    @classmethod
    def bare_domain(cls, v):
        if v is not None and "/" in v:
            raise ValueError("give a bare domain, like example.com")
        return v


class WeatherArgs(BaseModel):
    city: str
    unit: Literal["c", "f"] = "c"
    days: int = Field(default=1, ge=1, le=7)


class ToolCall(BaseModel):
    name: Literal["search", "weather"]
    arguments: dict


TOOLS = {"search": SearchArgs, "weather": WeatherArgs}

responses = [
    '{"name": "search", "arguments": {"query": "rust compilers", "limit": "10"}}',
    '{"name": "search", "arguments": {"query": "", "limit": 100, "sort": "popular"}}',
    '{"name": "weather", "arguments": {"city": "Oslo", "days": 3, "unit": "f"}}',
    '{"name": "weather", "arguments": {"days": 0}}',
    '{"name": "search", "arguments": {"query": "x", "site": "https://a.com/b"}}',
    '{"name": "calendar", "arguments": {}}',
    '{"name": "search", "arguments": {"query": "unterminated"',
]
for text in responses:
    try:
        call = ToolCall.model_validate_json(text)
        args = TOOLS[call.name].model_validate(call.arguments)
        print("ok", call.name, args.model_dump_json())
    except ValidationError as e:
        feedback = [{"field": ".".join(str(p) for p in err["loc"]), "problem": err["msg"]} for err in e.errors()]
        print("retry", e.title, json.dumps(feedback))
