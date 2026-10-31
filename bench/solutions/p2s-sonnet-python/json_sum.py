import sys, json

def f(x):
    if isinstance(x, bool) or x is None or isinstance(x, str):
        return 0
    if isinstance(x, (int, float)):
        return x
    if isinstance(x, list):
        return sum(f(i) for i in x)
    if isinstance(x, dict):
        if any(isinstance(v, str) and v == "red" for v in x.values()):
            return 0
        return sum(f(v) for v in x.values())
    return 0

print(f(json.loads(sys.stdin.read())))
