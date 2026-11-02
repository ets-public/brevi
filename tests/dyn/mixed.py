# Values whose type changes at run time (what typed inference rejects).
import json

def ev(node, env):
    kind = node[0]
    if kind == "num":
        return node[1]
    if kind == "var":
        return env[node[1]]
    if kind == "list":
        return [ev(x, env) for x in node[1]]
    a, b = ev(node[1], env), ev(node[2], env)
    if kind == "+":
        return a + b
    return a * b

env = {"x": 3, "s": "ab", "l": [1, "two", None]}
prog = [("+", ("num", 2), ("var", "x")), ("*", ("var", "s"), ("num", 3)), ("+", ("var", "l"), ("list", [("num", 4.5)]))]
for p in prog:
    v = ev(p, env)
    print(type(v).__name__, v)
data = json.loads('{"a": [1, 2.5, "x", null, true], "b": {"c": []}}')
print(data, json.dumps(data, sort_keys=True), json.dumps(data, indent=2))
rows = [("ann", 3, 2.5), ("bob", None, [1, 2])]
rows.sort(key=lambda r: r[0], reverse=True)
print(rows, {r[0]: r[1:] for r in rows})
try:
    print(1 + "a")
except TypeError as e:
    print("TypeError:", e)
