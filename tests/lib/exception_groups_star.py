def show(tag, f):
    try:
        f()
        print(tag, "no error")
    except BaseException as x:
        print(tag, type(x).__name__, repr(x), str(x))

def nested():
    try:
        raise ExceptionGroup("outer", [ValueError(1), ExceptionGroup("inner", [TypeError(2), ValueError(3)]), KeyError(4)])
    except* ValueError as e:
        print("V", repr(e))
    except* (TypeError, KeyError) as e:
        print("TK", repr(e), len(e.exceptions))
show("nested", nested)

def partial():
    try:
        raise ExceptionGroup("p", [ValueError(1), OSError(2)])
    except* ValueError:
        print("handled V")
show("partial", partial)

def bare():
    try:
        raise ExceptionGroup("b", [ValueError(1), TypeError(2)])
    except* ValueError:
        raise
    except* TypeError:
        print("T")
show("bare", bare)

def both_raise():
    try:
        raise ExceptionGroup("b", [ValueError(1), TypeError(2)])
    except* ValueError:
        raise RuntimeError("r1")
    except* TypeError:
        raise RuntimeError("r2")
show("both", both_raise)

def elsefin():
    try:
        print("body")
    except* ValueError:
        print("never")
    else:
        print("else")
    finally:
        print("finally")
show("elsefin", elsefin)

def fin_exc():
    try:
        raise ExceptionGroup("f", [ValueError(1)])
    except* TypeError:
        print("never")
    finally:
        print("finally runs")
show("fin_exc", fin_exc)

def badtype():
    try:
        raise ValueError(1)
    except* ExceptionGroup:
        pass
show("badtype", badtype)

class MyEG(ExceptionGroup):
    def derive(self, excs):
        return MyEG(self.message, excs)

def sub():
    try:
        raise MyEG("mine", [ValueError(1), TypeError(2)])
    except* ValueError as e:
        print("sub V", type(e).__name__, repr(e))
show("sub", sub)

g = ExceptionGroup("g", [ValueError(1), TypeError(2), ExceptionGroup("h", [ValueError(3)])])
print(repr(g.subgroup(ValueError)))
print(repr(g.subgroup(lambda x: isinstance(x, TypeError))))
print(g.subgroup(KeyError))
m, r = g.split((ValueError, TypeError))
print(repr(m), r)
print(g.subgroup(ExceptionGroup) is g, g.split(ExceptionGroup)[0] is g)
show("subgroup-instance", lambda: g.subgroup(g))
print(str(g), g.message, len(g.exceptions))
print(isinstance(g, Exception), isinstance(BaseExceptionGroup("x", [KeyboardInterrupt()]), Exception))
print(type(BaseExceptionGroup("x", [ValueError()])).__name__)
g.add_note("a note")
print(g.__notes__, repr(g.split(ValueError)[0].__notes__))
show("nest-base", lambda: ExceptionGroup("x", [KeyboardInterrupt()]))
show("kw", lambda: ExceptionGroup("x", [ValueError()], y=1))
show("empty", lambda: ExceptionGroup("x", []))

def handler_err():
    try:
        raise ExceptionGroup("he", [ValueError(1)])
    except* ValueError as e:
        e2 = e
        1 / 0
show("handler_err", handler_err)

def loop_star():
    out = []
    for i in range(3):
        try:
            if i == 1:
                raise ExceptionGroup("l", [ValueError(i)])
            out.append(i)
        except* ValueError as e:
            out.append(repr(e))
    print(out)
show("loop", loop_star)

def name_cleared():
    try:
        raise ExceptionGroup("n", [ValueError(1)])
    except* ValueError as e:
        pass
    try:
        print(e)
    except NameError as x:
        print("NameError", x)
show("cleared", name_cleared)
