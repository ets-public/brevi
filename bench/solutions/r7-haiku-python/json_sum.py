import sys
import json

def total(x):
    if isinstance(x, dict):
        if any(isinstance(v, str) and v == "red" for v in x.values()):
            return 0
        return sum(total(v) for v in x.values())
    if isinstance(x, list):
        return sum(total(v) for v in x)
    if isinstance(x, bool):
        return 0
    if isinstance(x, int):
        return x
    return 0

data = json.loads(sys.stdin.read())
print(total(data))
