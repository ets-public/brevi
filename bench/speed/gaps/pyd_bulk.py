# Validate many pydantic models in a loop (the bulk-validation speed gap).
import sys
import time
from typing import Optional

from pydantic import BaseModel


class Address(BaseModel):
    street: str
    city: str
    zip: str


class User(BaseModel):
    id: int
    name: str
    email: str
    age: Optional[int] = None
    tags: list[str] = []
    score: float = 0.0
    address: Address


n = int(sys.argv[1]) if len(sys.argv) > 1 else 50000
rows = []
for i in range(200):
    rows.append({"id": i, "name": "user%d" % i, "email": "u%d@x.org" % i, "age": i % 90 if i % 4 else None,
                 "tags": ["a", "b%d" % (i % 3)], "score": i * 0.5,
                 "address": {"street": "%d Main St" % i, "city": "Town", "zip": "%05d" % i}})
t = time.perf_counter()
s = 0
for i in range(n):
    u = User(**rows[i % 200])
    s += u.id + len(u.tags) + len(u.address.city)
dumped = User(**rows[7]).model_dump()
print(s, dumped)
print("time %.3f" % (time.perf_counter() - t), file=sys.stderr)
