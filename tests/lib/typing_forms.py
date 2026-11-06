from typing import TypedDict, Generic, TypeVar, Protocol, Optional, Union, List, Dict, Callable, Any, cast, overload, Final, ClassVar, TYPE_CHECKING, NewType, Literal, runtime_checkable
import typing as t
if TYPE_CHECKING:
    from os import PathLike
T = TypeVar("T")
K = TypeVar("K", bound="Base")
Vector = List[float]
JSON = Dict[str, Any]
UserId = NewType("UserId", int)
class Movie(TypedDict):
    title: str
    year: int
class Extra(Movie, total=False):
    rating: float
class Stack(Generic[T]):
    """LIFO."""
    def __init__(self) -> None:
        self.items: List[T] = []
    def push(self, x: T) -> None:
        self.items.append(x)
    def pop(self) -> T:
        return self.items.pop()
class Greeter(Protocol):
    def greet(self) -> str: ...
class En:
    def greet(self) -> str:
        return "hi"
class Pair(Generic[T, K]):
    def __init__(self, a: T, b: K) -> None:
        self.a, self.b = a, b
@overload
def f(x: int) -> int: ...
@overload
def f(x: str) -> str: ...
def f(x: Union[int, str]) -> Union[int, str]:
    return x * 2
def g(gr: Greeter, n: Optional[int] = None) -> str:
    return gr.greet() * (n or 1)
m: Movie = {"title": "x", "year": 1}
e = Extra(title="a", year=2)
s = Stack[int]()
s.push(1); s.push(2)
s2: Stack[str] = Stack()
print(m, e, type(e).__name__, s.pop(), f(3), f("a"), g(En(), 2), Stack.__doc__)
print(cast(int, "3"), cast("List[int]", [1]), UserId(5), t.cast(t.Optional[int], 4), TYPE_CHECKING, t.TYPE_CHECKING)
MAX: Final = 3
v: Vector = [1.0]
print(MAX, v, Pair(1, "x").b, isinstance(s, Stack))
