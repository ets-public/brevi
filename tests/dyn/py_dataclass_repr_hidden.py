# field(repr=False) is left out of a dataclass's repr, also in a subclass.
from dataclasses import dataclass, field


@dataclass
class A:
    owner: str
    secret: str = field(repr=False, default="s")


@dataclass
class B(A):
    n: int = 1


print(A("x"), [B("y")])
