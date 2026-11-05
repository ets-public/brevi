# globals()[name], globals().get(name) and name in globals() read the module globals live;
# names not bound yet, deleted names and except ... as names are absent, as in python3.
import json
from collections import Counter

LIMIT = 3


def cmd_hello(x):
    return f"hello {x}"


def cmd_count(x):
    return len(x)


def dispatch(name, arg):
    fn = globals().get("cmd_" + name)
    if fn is None:
        return "unknown " + name
    return fn(arg)


def run():
    for n in ["hello", "count", "nope"]:
        print(n, dispatch(n, "abc"))
    print("LIMIT" in globals(), "late" in globals(), "nothing" not in globals())
    print(globals()["LIMIT"], globals().get("missing", 7), globals().get("missing"))
    print(globals()["json"].dumps({"a": 1}), globals()["Counter"]("aab")["a"])
    print(globals()["__name__"])
    try:
        globals()["zzz"]
    except KeyError as e:
        print("KeyError", e)
    print(globals().get(5), 5 in globals())
    for v in ["x", "e"]:
        print(v, v in globals())


class Report:
    kind = "weekly"

    def make(self):
        return globals()["cmd_hello"](self.kind)


print("late" in globals(), "Report" in globals())
run()
late = 1
print("late" in globals(), Report().make())
x = 5
del x
print("x" in globals())
try:
    raise ValueError("v")
except ValueError as e:
    pass
print("e" in globals())
sq = [i * i for i in range(3)]
print("i" in globals(), "sq" in globals())
for k in range(2):
    pass
print("k" in globals(), globals()["k"])
def setter():
    global made
    made = 1
print("made" in globals()); setter(); print("made" in globals())
