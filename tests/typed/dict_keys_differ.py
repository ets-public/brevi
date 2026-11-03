# Dicts in one list with different keys: a key a dict lacks is left out, as in Python.
import json
res = []
for i in range(4):
    if i % 2:
        res.append({"id": i, "error": "bad"})
        continue
    res.append({"id": i, "total": str(i * 2)})
print(json.dumps(res))
print(res)
for r in res:
    print(r.get("error", "-"), "total" in r)
