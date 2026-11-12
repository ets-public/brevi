# sys.stdout replaced by the program (test.support.captured_output), and print() to a file object piece by piece
import sys, io
old = sys.stdout
sys.stdout = io.StringIO()
print("hidden")
print("x", "y", sep="-")
sys.stdout.write("w\n")
cap = sys.stdout.getvalue()
sys.stdout = old
print("shown", repr(cap))
print(sys.stdout is old, sys.__stdout__ is old)
def captured(name):
    orig = getattr(sys, name)
    setattr(sys, name, io.StringIO())
    try:
        print("inner")
        return getattr(sys, name).getvalue()
    finally:
        setattr(sys, name, orig)
print(repr(captured("stdout")))
class W:
    def __init__(self): self.parts = []
    def write(self, t): self.parts.append(t); return len(t)
    def flush(self): self.parts.append("<f>")
w = W(); sys.stdout = w
print("a", 1, end="!")
print("b", flush=True)
sys.stdout = sys.__stdout__
print(w.parts)
