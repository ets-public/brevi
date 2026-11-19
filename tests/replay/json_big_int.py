# A JSON int past 2**53 (json.loads keeps typed JSON numbers as f64): the typed build stops before it can
# round 9007199254740993 to ...992, and `brevi run` re-runs the program on the dynamic build.
import json

print("start")
d = json.loads('{"m": 9007199254740993, "n": 12345678901234567, "s": [-9007199254740993]}')
print(d["m"])
print(d["n"] + 1)
print(d["s"][0] - 1)
print(json.dumps(d))
