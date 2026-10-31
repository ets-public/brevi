import sys
import json


def total(x):
    if x is None or isinstance(x, bool):
        return 0
    if isinstance(x, (int, float)):
        return x
    if isinstance(x, list):
        return sum(total(i) for i in x)
    if isinstance(x, dict):
        if any(isinstance(v, str) and v == "red" for v in x.values()):
            return 0
        return sum(total(v) for v in x.values())
    return 0


data = json.loads(sys.stdin.read())
print(total(data))
