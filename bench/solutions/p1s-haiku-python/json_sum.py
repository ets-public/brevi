import sys
import json


def total(v):
    if isinstance(v, bool):
        return 0
    if isinstance(v, int):
        return v
    if isinstance(v, list):
        return sum(total(x) for x in v)
    if isinstance(v, dict):
        if any(isinstance(x, str) and x == "red" for x in v.values()):
            return 0
        return sum(total(x) for x in v.values())
    return 0


print(total(json.loads(sys.stdin.read())))
