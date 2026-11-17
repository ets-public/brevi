# A library module's function is what CPython says it is (src/dynpy/rt_modfn_data.rs, from CPython 3.13): a
# Python function (json.dumps, dataclasses.dataclass) is of type function, with its own __module__, binds as a
# method when a class holds it, and its repr is a function's; a C function (math.sqrt, heapq.heappush) is a
# builtin_function_or_method of its C module (_heapq), and keeps its CPython __name__ (bisect.bisect is
# bisect_right) and __doc__.
import bisect
import collections
import copy
import dataclasses
import functools
import heapq
import json
import math
import operator
import re


def mine():
    pass


for f in [dataclasses.dataclass, dataclasses.field, dataclasses.asdict, dataclasses.replace, json.dumps, json.loads,
          functools.reduce, functools.wraps, math.sqrt, collections.namedtuple, re.compile, re.sub, copy.deepcopy,
          heapq.heappush, bisect.bisect, operator.add, len]:
    print(type(f).__name__, f.__module__, f.__name__, f.__qualname__, (f.__doc__ or "").splitlines()[:1])
print(type(dataclasses.dataclass) is type(mine), type(math.sqrt) is type(len), json.dumps.__type_params__)
print(repr(math.sqrt), repr(json.dumps).startswith("<function dumps at 0x"))


class T:
    dumps = json.dumps
    sq = math.sqrt


try:
    T().dumps()
except TypeError as e:
    print("TypeError:", e)
print(T().sq(4.0))
try:
    math.sqrt.__defaults__
except AttributeError as e:
    print("AttributeError:", e)
# one CPython object under two names is one function here too
import pickle
print(bisect.bisect is bisect.bisect_right, pickle.loads(pickle.dumps(bisect.bisect)) is bisect.bisect_right, bisect.bisect([1, 2, 3], 2))
