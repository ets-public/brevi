# inspect.signature, Signature.parameters and inspect.Parameter on defs, a dataclass's __init__ and dataclasses.dataclass
# (expected = CPython 3.13); a dataclass field named self makes __init__'s first parameter __dataclass_self__
from dataclasses import dataclass, field, KW_ONLY
import dataclasses
import inspect
from inspect import signature, Parameter
class F: pass
@dataclass
class C:
    i: int
    j: str = 'x'
    k: F = field(default_factory=F)
    l: float = 3.0
    _: KW_ONLY
    m: int = 5
    z: complex = field(default=1j, init=False)
s = inspect.signature(C.__init__)
print(s, repr(s))
print(repr(s.parameters))
print(s.parameters is s.parameters, s.return_annotation, s.parameters['i'] is s.parameters['i'])
for p in s.parameters.values():
    print(repr(p), str(p), p.name, p.kind, repr(p.kind), p.kind.description, p.default is Parameter.empty, p.annotation, f"{p.kind}")
print(C.__init__.__annotations__)
print(signature(dataclasses.dataclass))
print(repr(Parameter.empty), Parameter.empty is inspect.Signature.empty, Parameter.KEYWORD_ONLY == 3, Parameter.KEYWORD_ONLY > Parameter.POSITIONAL_ONLY)
print(type(Parameter.VAR_KEYWORD), type(Parameter.VAR_KEYWORD).__mro__, isinstance(s.parameters['i'], Parameter), type(s.parameters['i']))
print(list(Parameter.KEYWORD_ONLY.__class__), Parameter.KEYWORD_ONLY.value, Parameter.KEYWORD_ONLY.name, hash(Parameter.VAR_KEYWORD)==hash(4))
@dataclass
class D:
    self: str
d = D('foo')
print(repr(d.self), d, list(signature(D.__init__).parameters))
def g(a, /, b: int = 2, *c, d, **e) -> str: pass
sg = signature(g)
print(sg, [(p.name, str(p.kind)) for p in sg.parameters.values()], sg.return_annotation)
print(signature(lambda: 0).return_annotation is inspect.Signature.empty)
try:
    signature()
except TypeError as e: print(e)
try:
    signature(g, foo=1)
except TypeError as e: print(e)
try:
    signature(3)
except TypeError as e: print(e)
