# Brevi's own pydantic: models, defaults, nesting, dumps, errors.
from typing import Optional, Union, Literal, Any
from pydantic import BaseModel, Field, ValidationError


class Address(BaseModel):
    street: str
    city: str = "Paris"
    zip_code: Optional[str] = None


class User(BaseModel):
    id: int
    name: str
    email: str = Field(alias="mail", default="none@example.com")
    age: int = Field(default=0, ge=0, le=150)
    score: float = 0
    tags: list[str] = []
    address: Optional[Address] = None
    friends: list[int] = Field(default_factory=list)
    role: Literal["admin", "user"] = "user"
    meta: dict[str, Any] = {}
    pair: tuple[int, str] = (0, "")
    either: Union[int, str] = 0


u = User(id="42", name="Ann", mail="ann@example.com", age="31", score=3, tags=("a", "b"), address={"street": "Main 1"})
print(repr(u))
print(u)
print(u.model_dump())
print(u.model_dump(by_alias=True))
print(u.model_dump(mode="json"))
print(u.model_dump_json())
print(u.model_dump_json(indent=2))
print(u.model_dump(include={"id", "address"}))
print(u.model_dump(exclude={"meta": True, "pair": True, "address": {"city"}}))
print(u.model_dump(exclude_defaults=True))
print(u.model_dump(exclude_unset=True))
print(u.model_dump(exclude_none=True))
print(sorted(u.model_fields_set))
print(u.address.city, u.tags, u.score, type(u.score).__name__)
print(u == User(id=42, name="Ann", mail="ann@example.com", age=31, score=3.0, tags=["a", "b"], address=Address(street="Main 1")))
v = u.model_copy(update={"name": "Bob"})
print(v.name, u.name, v.address is u.address, u.model_copy(deep=True).address is u.address)
print(User.model_validate({"id": 1, "name": "x", "either": "7"}).either)
print(User.model_validate({"id": 1, "name": "x", "either": 7.0}).either)
print(User.model_construct(id=5, name="raw"))
for k, f in User.model_fields.items():
    print(k, repr(f))
print(dict(u)["id"], [k for k, _ in u])

bad = [
    {},
    {"id": "x", "name": None},
    {"id": 1.5, "name": "n", "age": -1, "tags": "abc", "role": "root"},
    {"id": 1, "name": "n", "address": {"zip_code": 75001}},
    {"id": 1, "name": "n", "pair": [1], "friends": [1, "two", 3.0, 4.5]},
    {"id": 1, "name": "n", "either": [1], "meta": []},
    {"id": "1" * 60, "name": "n"},
]
for d in bad:
    try:
        User(**d)
    except ValidationError as e:
        print(e)
        print(e.error_count(), e.title)
        for err in e.errors():
            print(" ", err["type"], err["loc"], repr(err["msg"]), err.get("ctx"))
try:
    User.model_validate(5)
except ValidationError as e:
    print(e)
try:
    User.model_validate(["id", 1])
except ValidationError as e:
    print(e.errors(include_url=False))
