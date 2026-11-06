eg = ExceptionGroup("many", [ValueError(1), TypeError("t"), ExceptionGroup("inner", [KeyError("k"), ValueError(2)])])
print(str(eg), repr(eg))
print(type(eg).__name__, eg.message, len(eg.exceptions), isinstance(eg, Exception), isinstance(eg, BaseExceptionGroup))
m, r = eg.split(ValueError)
print(repr(m))
print(repr(r))
print(repr(eg.subgroup(lambda e: isinstance(e, KeyError))))
print(eg.subgroup(OSError))
b = BaseExceptionGroup("b", [ValueError(3)])
print(type(b).__name__)
b2 = BaseExceptionGroup("b", [KeyboardInterrupt()])
print(type(b2).__name__, isinstance(b2, Exception))
for bad in [lambda: ExceptionGroup("x", []), lambda: ExceptionGroup("x", [1]), lambda: ExceptionGroup(1, [ValueError()]), lambda: ExceptionGroup("x", [KeyboardInterrupt()])]:
    try:
        bad()
    except (TypeError, ValueError) as e:
        print(type(e).__name__, e)

def f(exc):
    try:
        raise exc
    except* ValueError as e:
        print("value:", repr(e))
    except* TypeError as e:
        print("type:", repr(e))

f(ValueError("plain"))
f(ExceptionGroup("g", [ValueError(1), TypeError(2), ValueError(3)]))
try:
    f(ExceptionGroup("g", [ValueError(1), KeyError(2)]))
except ExceptionGroup as e:
    print("left:", repr(e))
try:
    f(KeyError("naked"))
except KeyError as e:
    print("naked left:", repr(e))
try:
    try:
        raise ExceptionGroup("g", [ValueError(1), TypeError(2)])
    except* ValueError:
        raise
    except* TypeError as e:
        raise RuntimeError("new")
except ExceptionGroup as e:
    print("combined:", repr(e))
try:
    try:
        raise ValueError(5)
    except* ValueError:
        raise
except ExceptionGroup as e:
    print("bare reraise of plain:", repr(e))
try:
    try:
        raise ExceptionGroup("g", [ValueError(1)])
    except* ExceptionGroup:
        pass
except TypeError as e:
    print(e)
try:
    pass
except* ValueError:
    print("no")
else:
    print("else ran")
finally:
    print("finally ran")
