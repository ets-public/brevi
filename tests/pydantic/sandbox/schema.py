# parse_obj (pydantic v1's spelling) is not in Brevi's pydantic: under --sandbox the build is refused (CPython's
# pydantic would need --allow-py pydantic); outside it the program runs with CPython's pydantic.
from pydantic import BaseModel


class M(BaseModel):
    x: int


print(M.parse_obj({"x": 1}))
