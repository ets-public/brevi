# Brevi's own pydantic: model_validate_json and model_dump_json, as pydantic-core parses and writes JSON.
from typing import Any, Optional
from pydantic import BaseModel, ValidationError


class Point(BaseModel):
    x: float
    y: float = 0
    label: Optional[str] = None
    data: Any = None


print(Point.model_validate_json('{"x": 1, "y": 2.5, "label": "caf\\u00e9 \\ud83d\\ude00", "data": [1, 1.0, 1e3, -0.0, null, true, {"a": [ ]}]}'))
print(Point.model_validate_json(b'  {"x": "3.5"}  '))
p = Point(x=1e16, y=1e-7, data=[0.1, 1e15, 123456789.0, 1.5e300, 2.5e-10, float("inf"), float("nan"), "tab\there", "quote\"", "é", "\x01"])
print(p.model_dump_json())
print(p.model_dump_json(indent=4))
print(p.model_dump(mode="json"))
for s in ['', '{"x": 1,}', '[1, 2]', '{"x": nan}', '{"x" 1}', '{"x": 1} extra', '{"x": "a"}', '{"y": 1}', '{"x": 01}', '{"x": 1e}', '"text"', '{"x": [1}', '{\n  "x": tru\n}', '{"x": "\\q"}', 5]:
    try:
        Point.model_validate_json(s)
    except ValidationError as e:
        print(e)
