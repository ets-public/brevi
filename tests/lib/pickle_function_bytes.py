# pickle.dumps writes a def or class of the program as CPython does, byte for byte, at every protocol: a
# global (STACK_GLOBAL from protocol 4, getattr for a dotted name below it), its module name memoized once,
# and the same function memoized when it comes again; what loads back is the program's own object.
import io
import pickle


def f():
    return 1


class A:
    def m(self):
        return 2

    class B:
        def n(self):
            return 3


for p in range(pickle.HIGHEST_PROTOCOL + 1):
    print(p, pickle.dumps([f, A, A.m, A.B, A.B.n, f, A.B], p))
    x = pickle.loads(pickle.dumps([f, A.m, A.B.n, A.B], p))
    print(p, x[0](), x[1](A()), x[2](A.B()), x[3].__qualname__)
print(pickle.dumps(f))
print(pickle.dumps({"k": f, "j": (f, A)}))
b = io.BytesIO()
pickle.dump(A.B, b)
print(b.getvalue())
b.seek(0)
print(pickle.load(b).__qualname__)
# a dotted name in a GLOBAL opcode loads from protocol 4 on
print(pickle.loads(b"\x80\x04c__main__\nA.m\n.").__qualname__)
try:
    pickle.dumps(lambda: 1)
except pickle.PicklingError as e:
    print("PicklingError", str(e).split(": ", 1)[1])
