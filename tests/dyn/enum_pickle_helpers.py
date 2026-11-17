# enum's pickle helpers (pickle_by_global_name, pickle_by_enum_name, _make_class_unpicklable), and a Python
# library's exception class (pickle.PicklingError) against type(e) of one it raised (CPython 3.13.13 output)
import enum, pickle
from enum import Enum, IntEnum
print(enum.pickle_by_global_name is enum._reduce_ex_by_global_name)
print(enum._reduce_ex_by_global_name.__name__, enum.pickle_by_enum_name.__name__, enum.pickle_by_enum_name.__module__)
class Color(Enum):
    RED = 1
    GREEN = 2
print(enum.pickle_by_global_name(Color.RED, 2), enum.pickle_by_enum_name(Color.GREEN, 4))
Color.__reduce_ex__ = enum.pickle_by_enum_name
print(Color.RED.__reduce_ex__(3))
print(pickle.loads(pickle.dumps(Color.GREEN)) is Color.GREEN)
class Shape(IntEnum):
    SQ = 1
Shape.__reduce_ex__ = enum.pickle_by_global_name
try:
    pickle.dumps(Shape.SQ)
except Exception as e:
    print(type(e).__name__, e)
globals()['SQ'] = Shape.SQ
print(pickle.loads(pickle.dumps(Shape.SQ)) is Shape.SQ)
d = {}
print(enum._make_class_unpicklable(d))
print(sorted(d), d['__module__'], d['__reduce_ex__'].__name__, d['__reduce_ex__'].__qualname__)
try:
    d['__reduce_ex__'](Color.RED, 2)
except TypeError as e:
    print('TE', e)
try:
    enum.pickle_by_enum_name(Color.RED)
except TypeError as e:
    print('TE', e)
class W(Enum):
    A = 1
enum._make_class_unpicklable(W)
print(W.__module__)
try:
    pickle.dumps(W.A)
except TypeError as e:
    print('TE', e)
try:
    pickle.dumps([1, W.A])
except TypeError as e:
    print('TE', e)
try:
    pickle.dumps(W)
except pickle.PicklingError as e:
    print('PE', type(e).__name__)
    print(type(e).__mro__)
    print(type(e).__bases__)
    print(type(e).__qualname__, type(e).__name__)
    print(type(e).__mro__[1] is pickle.PickleError, issubclass(type(e).__mro__[1], Exception))
    try:
        raise type(e)('again')
    except pickle.PickleError as e2:
        print('caught', type(e2).__name__, e2)
print(enum.pickle_by_enum_name(proto=1, self=Color.RED))
try:
    enum.pickle_by_enum_name(Color.RED, 1, 2)
except TypeError as e:
    print('TE', e)
import json
try:
    json.loads('{')
except json.JSONDecodeError as e:
    print(type(e).__mro__, e.msg, e.pos)

# ---- the class pickle raised, against the one the program imported
from pickle import PicklingError, PickleError, dumps
import pickle  # again
class C: pass
class D(Exception): pass
C.__module__ = '<unknown>'
try:
    dumps(C)
except Exception as e:
    t = type(e)
    print(t)
    print(t is PicklingError, PicklingError is t, t is PickleError, t == PicklingError, t != PicklingError)
    print(isinstance(e, PicklingError), isinstance(e, PickleError))
    print(issubclass(PicklingError, Exception), issubclass(PicklingError, BaseException), issubclass(PicklingError, ValueError))
    print(issubclass(t, Exception), issubclass(t, PicklingError), issubclass(t, PickleError), issubclass(t, (ValueError, PickleError)))
    print(issubclass(PicklingError, PickleError), issubclass(PickleError, PicklingError), issubclass(PicklingError, D), issubclass(PicklingError, C))
    print(issubclass(PicklingError, t), issubclass(PickleError, t), issubclass(t, t))
    print(issubclass(D, PicklingError), issubclass(ValueError, PickleError))
    print(t in [PicklingError], PicklingError in (t,))
try:
    issubclass(pickle.dumps, Exception)
except TypeError as e:
    print('TE', e)
try:
    dumps(C)
except Exception as e:
    t = type(e)
    print(hash(t) == hash(PicklingError))
    d = {t: 1}
    print(d.get(PicklingError), {PicklingError: 2}.get(t), PicklingError in {t}, len({t, PicklingError}))
