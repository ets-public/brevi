from typing import Generic, TypeVar, Protocol, Iterator, Callable, Optional, Literal, Any
import collections, collections.abc, re
T=TypeVar("T"); U=TypeVar("U")
class G(Generic[T]): pass
class H(Generic[T, U]): pass
class B(G): pass
class C(G[int]): pass
class P(Protocol[T]): pass
class Q(Protocol): pass
class D(dict): pass
class E: pass

try:
    def f0(x: G[int]) -> None: pass
    print('G[int]', 'ok')
except Exception as x:
    print('G[int]', type(x).__name__, x)
try:
    y0: G[int]
    print('G[int]', 'ok')
except Exception as x:
    print('G[int]', type(x).__name__, x)
try:
    def f1(x: G[int,str]) -> None: pass
    print('G[int,str]', 'ok')
except Exception as x:
    print('G[int,str]', type(x).__name__, x)
try:
    y1: G[int,str]
    print('G[int,str]', 'ok')
except Exception as x:
    print('G[int,str]', type(x).__name__, x)
try:
    def f2(x: G[()]) -> None: pass
    print('G[()]', 'ok')
except Exception as x:
    print('G[()]', type(x).__name__, x)
try:
    y2: G[()]
    print('G[()]', 'ok')
except Exception as x:
    print('G[()]', type(x).__name__, x)
try:
    def f3(x: H[int]) -> None: pass
    print('H[int]', 'ok')
except Exception as x:
    print('H[int]', type(x).__name__, x)
try:
    y3: H[int]
    print('H[int]', 'ok')
except Exception as x:
    print('H[int]', type(x).__name__, x)
try:
    def f4(x: H[int,str,str]) -> None: pass
    print('H[int,str,str]', 'ok')
except Exception as x:
    print('H[int,str,str]', type(x).__name__, x)
try:
    y4: H[int,str,str]
    print('H[int,str,str]', 'ok')
except Exception as x:
    print('H[int,str,str]', type(x).__name__, x)
try:
    def f5(x: B[int]) -> None: pass
    print('B[int]', 'ok')
except Exception as x:
    print('B[int]', type(x).__name__, x)
try:
    y5: B[int]
    print('B[int]', 'ok')
except Exception as x:
    print('B[int]', type(x).__name__, x)
try:
    def f6(x: P[int]) -> None: pass
    print('P[int]', 'ok')
except Exception as x:
    print('P[int]', type(x).__name__, x)
try:
    y6: P[int]
    print('P[int]', 'ok')
except Exception as x:
    print('P[int]', type(x).__name__, x)
try:
    def f7(x: Q[int]) -> None: pass
    print('Q[int]', 'ok')
except Exception as x:
    print('Q[int]', type(x).__name__, x)
try:
    y7: Q[int]
    print('Q[int]', 'ok')
except Exception as x:
    print('Q[int]', type(x).__name__, x)
try:
    def f8(x: G[1]) -> None: pass
    print('G[1]', 'ok')
except Exception as x:
    print('G[1]', type(x).__name__, x)
try:
    y8: G[1]
    print('G[1]', 'ok')
except Exception as x:
    print('G[1]', type(x).__name__, x)
try:
    def f9(x: G[(int,)]) -> None: pass
    print('G[(int,)]', 'ok')
except Exception as x:
    print('G[(int,)]', type(x).__name__, x)
try:
    y9: G[(int,)]
    print('G[(int,)]', 'ok')
except Exception as x:
    print('G[(int,)]', type(x).__name__, x)
try:
    def f10(x: H[(int,str)]) -> None: pass
    print('H[(int,str)]', 'ok')
except Exception as x:
    print('H[(int,str)]', type(x).__name__, x)
try:
    y10: H[(int,str)]
    print('H[(int,str)]', 'ok')
except Exception as x:
    print('H[(int,str)]', type(x).__name__, x)
try:
    def f11(x: D[int]) -> None: pass
    print('D[int]', 'ok')
except Exception as x:
    print('D[int]', type(x).__name__, x)
try:
    y11: D[int]
    print('D[int]', 'ok')
except Exception as x:
    print('D[int]', type(x).__name__, x)
try:
    def f12(x: E[int]) -> None: pass
    print('E[int]', 'ok')
except Exception as x:
    print('E[int]', type(x).__name__, x)
try:
    y12: E[int]
    print('E[int]', 'ok')
except Exception as x:
    print('E[int]', type(x).__name__, x)
try:
    def f13(x: collections.OrderedDict[str,int]) -> None: pass
    print('collections.OrderedDict[str,int]', 'ok')
except Exception as x:
    print('collections.OrderedDict[str,int]', type(x).__name__, x)
try:
    y13: collections.OrderedDict[str,int]
    print('collections.OrderedDict[str,int]', 'ok')
except Exception as x:
    print('collections.OrderedDict[str,int]', type(x).__name__, x)
try:
    def f14(x: collections.defaultdict[str,int]) -> None: pass
    print('collections.defaultdict[str,int]', 'ok')
except Exception as x:
    print('collections.defaultdict[str,int]', type(x).__name__, x)
try:
    y14: collections.defaultdict[str,int]
    print('collections.defaultdict[str,int]', 'ok')
except Exception as x:
    print('collections.defaultdict[str,int]', type(x).__name__, x)
try:
    def f15(x: collections.deque[int]) -> None: pass
    print('collections.deque[int]', 'ok')
except Exception as x:
    print('collections.deque[int]', type(x).__name__, x)
try:
    y15: collections.deque[int]
    print('collections.deque[int]', 'ok')
except Exception as x:
    print('collections.deque[int]', type(x).__name__, x)
try:
    def f16(x: collections.Counter[str]) -> None: pass
    print('collections.Counter[str]', 'ok')
except Exception as x:
    print('collections.Counter[str]', type(x).__name__, x)
try:
    y16: collections.Counter[str]
    print('collections.Counter[str]', 'ok')
except Exception as x:
    print('collections.Counter[str]', type(x).__name__, x)
try:
    def f17(x: re.Pattern[str]) -> None: pass
    print('re.Pattern[str]', 'ok')
except Exception as x:
    print('re.Pattern[str]', type(x).__name__, x)
try:
    y17: re.Pattern[str]
    print('re.Pattern[str]', 'ok')
except Exception as x:
    print('re.Pattern[str]', type(x).__name__, x)
try:
    def f18(x: type[int]) -> None: pass
    print('type[int]', 'ok')
except Exception as x:
    print('type[int]', type(x).__name__, x)
try:
    y18: type[int]
    print('type[int]', 'ok')
except Exception as x:
    print('type[int]', type(x).__name__, x)
try:
    def f19(x: collections.abc.Mapping[str,int]) -> None: pass
    print('collections.abc.Mapping[str,int]', 'ok')
except Exception as x:
    print('collections.abc.Mapping[str,int]', type(x).__name__, x)
try:
    y19: collections.abc.Mapping[str,int]
    print('collections.abc.Mapping[str,int]', 'ok')
except Exception as x:
    print('collections.abc.Mapping[str,int]', type(x).__name__, x)
try:
    def f20(x: collections.abc.Hashable[int]) -> None: pass
    print('collections.abc.Hashable[int]', 'ok')
except Exception as x:
    print('collections.abc.Hashable[int]', type(x).__name__, x)
try:
    y20: collections.abc.Hashable[int]
    print('collections.abc.Hashable[int]', 'ok')
except Exception as x:
    print('collections.abc.Hashable[int]', type(x).__name__, x)
try:
    def f21(x: collections.abc.Callable[[int],str]) -> None: pass
    print('collections.abc.Callable[[int],str]', 'ok')
except Exception as x:
    print('collections.abc.Callable[[int],str]', type(x).__name__, x)
try:
    y21: collections.abc.Callable[[int],str]
    print('collections.abc.Callable[[int],str]', 'ok')
except Exception as x:
    print('collections.abc.Callable[[int],str]', type(x).__name__, x)
try:
    def f22(x: collections.abc.Iterator[int]) -> None: pass
    print('collections.abc.Iterator[int]', 'ok')
except Exception as x:
    print('collections.abc.Iterator[int]', type(x).__name__, x)
try:
    y22: collections.abc.Iterator[int]
    print('collections.abc.Iterator[int]', 'ok')
except Exception as x:
    print('collections.abc.Iterator[int]', type(x).__name__, x)
try:
    def f23(x: Iterator[int]) -> None: pass
    print('Iterator[int]', 'ok')
except Exception as x:
    print('Iterator[int]', type(x).__name__, x)
try:
    y23: Iterator[int]
    print('Iterator[int]', 'ok')
except Exception as x:
    print('Iterator[int]', type(x).__name__, x)
try:
    def f24(x: Callable[..., int]) -> None: pass
    print('Callable[..., int]', 'ok')
except Exception as x:
    print('Callable[..., int]', type(x).__name__, x)
try:
    y24: Callable[..., int]
    print('Callable[..., int]', 'ok')
except Exception as x:
    print('Callable[..., int]', type(x).__name__, x)
try:
    def f25(x: Optional['G']) -> None: pass
    print("Optional['G']", 'ok')
except Exception as x:
    print("Optional['G']", type(x).__name__, x)
try:
    y25: Optional['G']
    print("Optional['G']", 'ok')
except Exception as x:
    print("Optional['G']", type(x).__name__, x)
try:
    def f26(x: Literal[1,2]) -> None: pass
    print('Literal[1,2]', 'ok')
except Exception as x:
    print('Literal[1,2]', type(x).__name__, x)
try:
    y26: Literal[1,2]
    print('Literal[1,2]', 'ok')
except Exception as x:
    print('Literal[1,2]', type(x).__name__, x)
try:
    def f27(x: int | None) -> None: pass
    print('int | None', 'ok')
except Exception as x:
    print('int | None', type(x).__name__, x)
try:
    y27: int | None
    print('int | None', 'ok')
except Exception as x:
    print('int | None', type(x).__name__, x)
try:
    def f28(x: G | None) -> None: pass
    print('G | None', 'ok')
except Exception as x:
    print('G | None', type(x).__name__, x)
try:
    y28: G | None
    print('G | None', 'ok')
except Exception as x:
    print('G | None', type(x).__name__, x)
try:
    def f29(x: list[int] | None) -> None: pass
    print('list[int] | None', 'ok')
except Exception as x:
    print('list[int] | None', type(x).__name__, x)
try:
    y29: list[int] | None
    print('list[int] | None', 'ok')
except Exception as x:
    print('list[int] | None', type(x).__name__, x)
try:
    def f30(x: frozenset[int]) -> None: pass
    print('frozenset[int]', 'ok')
except Exception as x:
    print('frozenset[int]', type(x).__name__, x)
try:
    y30: frozenset[int]
    print('frozenset[int]', 'ok')
except Exception as x:
    print('frozenset[int]', type(x).__name__, x)
try:
    def f31(x: object[int]) -> None: pass
    print('object[int]', 'ok')
except Exception as x:
    print('object[int]', type(x).__name__, x)
try:
    y31: object[int]
    print('object[int]', 'ok')
except Exception as x:
    print('object[int]', type(x).__name__, x)
try:
    def f32(x: str[int]) -> None: pass
    print('str[int]', 'ok')
except Exception as x:
    print('str[int]', type(x).__name__, x)
try:
    y32: str[int]
    print('str[int]', 'ok')
except Exception as x:
    print('str[int]', type(x).__name__, x)
try:
    def f33(x: bool[int]) -> None: pass
    print('bool[int]', 'ok')
except Exception as x:
    print('bool[int]', type(x).__name__, x)
try:
    y33: bool[int]
    print('bool[int]', 'ok')
except Exception as x:
    print('bool[int]', type(x).__name__, x)
try:
    def f34(x: Any) -> None: pass
    print('Any', 'ok')
except Exception as x:
    print('Any', type(x).__name__, x)
try:
    y34: Any
    print('Any', 'ok')
except Exception as x:
    print('Any', type(x).__name__, x)
try:
    def f35(x: Optional[Any]) -> None: pass
    print('Optional[Any]', 'ok')
except Exception as x:
    print('Optional[Any]', type(x).__name__, x)
try:
    y35: Optional[Any]
    print('Optional[Any]', 'ok')
except Exception as x:
    print('Optional[Any]', type(x).__name__, x)
try:
    def f36(x: dict[str, Any]) -> None: pass
    print('dict[str, Any]', 'ok')
except Exception as x:
    print('dict[str, Any]', type(x).__name__, x)
try:
    y36: dict[str, Any]
    print('dict[str, Any]', 'ok')
except Exception as x:
    print('dict[str, Any]', type(x).__name__, x)
try:
    zz: undefined_name = 1
except NameError as e:
    print("NameError", e)
class K:
    try:
        w: also_undefined
    except NameError as e:
        print("class NameError", e)
try:
    def later(a: Later) -> int:
        return 1
except NameError as e:
    print("def NameError", e)
class Later:
    pass
