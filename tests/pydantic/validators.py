# Brevi's own pydantic: field and model validators, computed fields, Annotated validators.
from typing import Annotated
from pydantic import BaseModel, Field, ValidationError, field_validator, model_validator, computed_field, AfterValidator, BeforeValidator, ValidationInfo


def even(v):
    if v % 2:
        raise ValueError(f"{v} is odd")
    return v


Even = Annotated[int, AfterValidator(even)]


class Order(BaseModel):
    item: str
    qty: int = 1
    price: float
    codes: list[Even] = []
    note: Annotated[str, BeforeValidator(lambda v: str(v).strip())] = ""

    @field_validator("item")
    @classmethod
    def item_name(cls, v):
        if not v:
            raise ValueError("item must not be empty")
        return v.title()

    @field_validator("qty", mode="before")
    @classmethod
    def qty_text(cls, v):
        if isinstance(v, str) and v.endswith("x"):
            return v[:-1]
        return v

    @field_validator("price")
    @classmethod
    def price_ok(cls, v, info: ValidationInfo):
        assert v >= 0, "price is negative"
        if info.data.get("qty", 0) > 100:
            raise ValueError(f"too many {info.data['item']} for {info.field_name}")
        return round(v, 2)

    @model_validator(mode="before")
    @classmethod
    def unwrap(cls, data):
        if isinstance(data, dict) and "order" in data:
            return data["order"]
        return data

    @model_validator(mode="after")
    def check_total(self):
        if self.total > 1000:
            raise ValueError("order too large")
        return self

    @computed_field
    @property
    def total(self) -> float:
        return round(self.qty * self.price, 2)


print(Order(item="apple", qty="3x", price=1.234, codes=[2, "4"], note="  hi  "))
print(Order.model_validate({"order": {"item": "pear", "price": 2}}).model_dump())
print(Order(item="fig", price=1).model_dump_json())
for d in [
    {"item": "", "price": 1},
    {"item": "a", "price": -1},
    {"item": "a", "qty": 101, "price": 1},
    {"item": "a", "qty": 10, "price": 200},
    {"item": "a", "price": 1, "codes": [1, 2, 3]},
    {"item": "a", "price": "cheap", "qty": "many"},
]:
    try:
        Order(**d)
    except ValidationError as e:
        print(e)
        print([(x["type"], x["loc"], x["msg"]) for x in e.errors()])


class Tags(BaseModel):
    names: list[str]
    counts: dict[str, int] = {}

    @field_validator("*", mode="before")
    @classmethod
    def split(cls, v):
        if isinstance(v, str):
            return v.split(",")
        return v

    @field_validator("names", mode="wrap")
    @classmethod
    def keep(cls, v, handler):
        try:
            return handler(v)
        except ValidationError:
            return ["?"]

    @field_validator("names", mode="plain")
    @classmethod
    def plain(cls, v):
        return sorted(v)


print(Tags(names="b,a,c"))
print(Tags(names=[3, 1, 2]))


class Wrapped(BaseModel):
    x: int

    @model_validator(mode="wrap")
    @classmethod
    def around(cls, data, handler):
        if data == "zero":
            data = {"x": 0}
        m = handler(data)
        return m


print(Wrapped.model_validate("zero"), Wrapped.model_validate({"x": "5"}))
try:
    Wrapped.model_validate("nope")
except ValidationError as e:
    print(e)
