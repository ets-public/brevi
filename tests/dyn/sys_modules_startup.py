# sys.modules as CPython 3.13 has it: the modules it loads before the script runs are there from the start,
# a library module once the program imports it, nothing else. The program imports only quiet modules (sys,
# math, itertools: their import adds no other module), so every answer is sure.
import sys
print("os" in sys.modules, "os.path" in sys.modules, "time" in sys.modules, "io" in sys.modules, "abc" in sys.modules)
print("sys" in sys.modules, "builtins" in sys.modules, "site" in sys.modules, "codecs" in sys.modules)
print("json" in sys.modules, "re" in sys.modules, "math" in sys.modules, "numpy" in sys.modules, "nosuchmodule" not in sys.modules)
print(sys.modules.get("json"), sys.modules.get("json", 7), sys.modules.get("math", 7))
try:
    sys.modules["math"]
except KeyError as e:
    print("KeyError", e)
import math
print("math" in sys.modules, sys.modules["math"] is math, sys.modules.get("math") is math)
t = sys.modules["time"]
import time
print(t is time, t.time() > 0, sys.modules["time"] is time, sys.modules["sys"] is sys)
print(sys.modules[__name__] is sys.modules["__main__"], __name__ in sys.modules)
def later():
    import itertools
    return "itertools" in sys.modules
print("itertools" in sys.modules, later(), "itertools" in sys.modules)
sys.modules["mine"] = 42
print("mine" in sys.modules, sys.modules["mine"], sys.modules.get("mine"))
for k in ["os", "math", "json", "mine", "never"]:
    print(k, k in sys.modules, k not in sys.modules)
print([sys.modules.get(k, "-") for k in ["math", "json", "mine", "never"]][1:])
