# classmethod and staticmethod of a functools.partial: a classmethod binds the class as the partial's next
# argument, a staticmethod gives the partial itself (no FutureWarning, its own __get__ never runs).
import functools, warnings

def capture(*args, **kw):
    return args, kw

class A:
    cm = classmethod(functools.partial(capture, 1, a=2))
    sm = staticmethod(functools.partial(capture, 1, a=2))

with warnings.catch_warnings(record=True) as ws:
    warnings.simplefilter("always")
    print(A.cm(3) == ((1, A, 3), {"a": 2}), A().cm(3) == ((1, A, 3), {"a": 2}))
    print(A.sm(3, b=4), A().sm(3))
print(len(ws))
