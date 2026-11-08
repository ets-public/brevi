# TypeAdapter: validating, dumping and the JSON schema of types that are not models
from typing import Annotated, Dict, List, Literal, Optional, Union

from pydantic import BaseModel, Field, TypeAdapter, ValidationError


class Item(BaseModel):
    name: str
    price: float = 0.0


ints = TypeAdapter(list[int])
print(ints.validate_python([1, "2", 3.0]))
print(ints.validate_json("[4, 5]"))
print(ints.dump_json([1, 2]), ints.dump_python([1, 2]))
print(ints.json_schema())
try:
    ints.validate_python([1, "x", None])
except ValidationError as e:
    print(e)
    print(e.errors())

items = TypeAdapter(List[Item])
got = items.validate_python([{"name": "a"}, {"name": "b", "price": "2.5"}])
print(got)
print(items.dump_python(got), items.dump_json(got, indent=2))
print(items.json_schema())

opt = TypeAdapter(Optional[int])
print(opt.validate_python(None), opt.validate_python("7"), opt.json_schema())
mapping = TypeAdapter(Dict[str, Union[int, str]])
print(mapping.validate_json('{"a": 1, "b": "x"}'), mapping.json_schema())
lit = TypeAdapter(Literal["yes", "no"])
print(lit.validate_python("yes"))
try:
    lit.validate_python("maybe")
except ValidationError as e:
    print(e)
small = TypeAdapter(Annotated[int, Field(ge=0, le=10)])
print(small.validate_python(3), small.json_schema())
try:
    small.validate_python(11)
except ValidationError as e:
    print(e)
model = TypeAdapter(Item)
print(model.validate_python({"name": "z"}), model.json_schema())
print(TypeAdapter(str).validate_python("s"), TypeAdapter(float).dump_json(1.5), TypeAdapter(bool).validate_strings("true"))
try:
    TypeAdapter(int).validate_python("abc")
except ValidationError as e:
    print(e.error_count(), e.title, e)
print(TypeAdapter(tuple[int, str]).validate_python(["1", "a"]), TypeAdapter(set[str]).json_schema())
