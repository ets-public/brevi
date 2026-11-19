# JSON ints up to 2**53 are exact in the typed build's f64 JSON numbers: they stay ints (from 1e15 too).
import json

d = json.loads('{"m": 1234567890123456, "n": -9007199254740992, "p": 9007199254740992, "f": 1e20, "g": 3.0}')
print(d["m"] + 1, d["n"] - 0, d["p"] - 1)
print(json.dumps(d))
x = d["m"] * 1
print(x)
total = 0
for k in ["m", "p"]:
    total += d[k]
print(total)
