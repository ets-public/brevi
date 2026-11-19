# An int past 2**53 put in a value of mixed type (a dict or list json.dumps writes) is not rounded.
import json

n = 9007199254740993
print(json.dumps({"a": n, "b": "x"}))
print(json.dumps([n, 1]))
