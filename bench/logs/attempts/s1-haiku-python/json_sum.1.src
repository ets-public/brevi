import sys
import json


def total(v):
    if isinstance(v, bool) or v is None:
        return 0
    if isinstance(v, (int, float)):
        return v
    if isinstance(v, str):
        return 0
    if isinstance(v, list):
        return sum(total(x) for x in v)
    if isinstance(v, dict):
        if any(isinstance(x, str) and x == "red" for x in v.values()):
            return 0
        return sum(total(x) for x in v.values())
    return 0


data = json.loads(sys.stdin.read())
result = total(data)
if isinstance(result, float) and result.is_integer():
    result = int(result)
print(result)
