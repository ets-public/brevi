class A(Exception):
    pass
try:
    try:
        raise ValueError("v")
    except ValueError as e:
        raise A("a") from e
except A as x:
    print(type(x.__cause__).__name__, x.__cause__, x.__suppress_context__)
try:
    raise A("b") from None
except A as x:
    print(x.__cause__, x.__suppress_context__)
try:
    raise A("c")
except A as x:
    print(x.__cause__, x.__suppress_context__)
try:
    raise A("d") from KeyError
except A as x:
    print(repr(x.__cause__))
try:
    raise KeyError("k") from ValueError(1)
except KeyError as x:
    print(repr(x.__cause__))
