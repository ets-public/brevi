# functools.singledispatch's register reads a class from the def's annotation: an ABC (collections.abc.Mapping)
# and a string naming one through the module's globals, as typing.get_type_hints evaluates it.
import functools, collections.abc

@functools.singledispatch
def kind(arg):
    return "base"

@kind.register
def _(arg: collections.abc.Mapping):
    return "mapping"

@kind.register
def _(arg: "collections.abc.Sequence"):
    return "sequence"

@kind.register
def _(arg: "int"):
    return "int"

print(kind({}), kind([]), kind((1,)), kind(1), kind(True), kind(1.5))
