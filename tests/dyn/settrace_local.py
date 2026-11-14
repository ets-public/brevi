# sys.settrace in a dynamic build: a local trace function, a None result keeping f_trace, a raising trace function
# turning tracing off, and settrace(None) part way through a frame
import sys

seen = [0]


def local(frame, event, arg):
    print("  local", event, frame.f_lineno - frame.f_code.co_firstlineno)
    if event == "line":
        return None
    return local


def glob(frame, event, arg):
    print("global", event, frame.f_code.co_name)
    if frame.f_code.co_name == "quiet":
        return None
    return local


def work(n):
    a = n * 2
    a += 1
    return a


def quiet():
    return work(1)


def stopper(frame, event, arg):
    seen[0] += 1
    print("stopper", event, frame.f_code.co_name, seen[0])
    if seen[0] == 3:
        raise RuntimeError("stop")
    return stopper


def halfway():
    a = 1
    sys.settrace(None)
    a = 2
    return a


sys.settrace(glob)
print(work(3))
print(quiet())
sys.settrace(stopper)
try:
    work(4)
except RuntimeError as e:
    print("raised", e, sys.gettrace())
sys.settrace(glob)
print(halfway(), sys.gettrace())
