# model_json_schema: pydantic's JSON schema of models (titles, defaults, constraints, $defs for nested models and
# enums, Optional, unions, literals, tuples, dicts, sets, aliases, docstrings, recursion, extra="forbid")
import json
from enum import Enum, IntEnum
from typing import Annotated, Any, Dict, List, Literal, Optional, Tuple, Union

from pydantic import BaseModel, ConfigDict, Field, PositiveInt


class Color(Enum):
    RED = "red"
    BLUE = "blue"


class Level(IntEnum):
    """How loud."""
    LOW = 1
    HIGH = 2


class Pet(BaseModel):
    """A pet.

    With a second line."""
    name: str
    age: int = 3


class User(BaseModel):
    first_name: str = Field(min_length=1, max_length=40, description="Given name")
    score: float = Field(0.5, ge=0, le=1)
    count: int = Field(gt=0, lt=100, multiple_of=5)
    tags: list[str] = []
    pets: List[Pet]
    best: Optional[Pet] = None
    either: Union[Pet, int] = 1
    color: Color = Color.RED
    level: Optional[Level] = None
    mode: Literal["a", "b"]
    one: Literal[1]
    mixed: Literal["x", 2] = "x"
    pair: tuple[int, str]
    many: Tuple[float, ...] = ()
    meta: dict[str, int] = {}
    loose: Dict = {}
    u: Union[int, str]
    maybe: int | None = None
    raw: bytes = b"x"
    s: set[int] = set()
    anything: Any = None
    pos: PositiveInt = 1
    code: Annotated[str, Field(pattern=r"^[A-Z]+$", examples=["AB"])] = "A"
    nick: str = Field("n", alias="nickName", title="Nickname")
    old: int = Field(0, deprecated=True)
    extra_info: str = Field("", json_schema_extra={"x-ui": "hidden"})


class Strict(BaseModel):
    model_config = ConfigDict(extra="forbid", title="StrictThing")
    x: int


class Node(BaseModel):
    value: int
    children: list["Node"] = []


print(json.dumps(User.model_json_schema(), indent=2))
print(json.dumps(User.model_json_schema(by_alias=False)["properties"]["nick"]))
print(User.model_json_schema(ref_template="#/components/schemas/{model}")["properties"]["pets"])
print(Strict.model_json_schema())
print(json.dumps(Node.model_json_schema()))
print(Pet.model_json_schema())
