# Calling the class of a builtin iterator (type(iter([]))()) raises CPython's TypeError: those types have no
# constructor. reversed() of a deque is CPython's collections._deque_reverse_iterator, not builtins.reversed.
import collections

def gen():
    yield 1

cases = [
    ("list", iter([1])), ("tuple", iter((1,))), ("str", iter("ab")), ("str_u", iter("é")),
    ("bytes", iter(b"ab")), ("bytearray", iter(bytearray(b"a"))), ("set", iter({1})),
    ("frozenset", iter(frozenset({1}))), ("dict", iter({1: 2})), ("values", iter({1: 2}.values())),
    ("items", iter({1: 2}.items())), ("range", iter(range(3))), ("bigrange", iter(range(10**20))),
    ("rev_list", reversed([1])), ("rev_dict", reversed({1: 2})), ("callable", iter(lambda: 1, 1)),
    ("gen", gen()), ("genexpr", (x for x in [1])), ("seq", iter(collections.UserList([1]))),
    ("odict", iter(collections.OrderedDict(a=1))), ("memview", iter(memoryview(b"a"))),
]
for name, it in cases:
    t = type(it)
    for args in [(), ([1],)]:
        try:
            t(*args)
            print(name, len(args), "made")
        except TypeError as e:
            print(name, len(args), "TypeError", e)

d = collections.deque([1, 2, 3])
r = reversed(d)
print(type(r).__name__, type(r).__module__, type(r).__qualname__)
print(isinstance(r, reversed), type(r) is type(reversed(collections.deque())), type(r) is type(iter(d)))
print(list(r), list(d.__reversed__()), type(d.__reversed__()).__name__)
print(type(reversed([1, 2])).__name__)
