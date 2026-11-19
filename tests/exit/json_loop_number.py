# A loop over a JSON number stops with TypeError as in Python (the typed build looped zero times).
import json

d = json.loads('{"n": 5, "xs": [1, 2]}')
for x in d["xs"]:
    print(x)
for x in d["n"]:
    print("never", x)
print("not reached")
