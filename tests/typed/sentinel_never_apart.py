# MISSING = object() where every test of it also tests None: swapped for None.
import json
root = json.loads('{"a": {"b": null, "c": 2}, "s": "x"}')
MISSING = object()
def lookup(path, ctx):
    v = ctx
    for s in path.split('.'):
        if isinstance(v, dict) and s in v:
            v = v[s]
        else:
            return MISSING
    return v
def text(v):
    if v is MISSING or v is None:
        return ''
    if isinstance(v, bool):
        return 'true' if v else 'false'
    if isinstance(v, str):
        return v
    return json.dumps(v)
for p in ["a.b", "a.c", "a.z", "s", "q"]:
    print(p, repr(text(lookup(p, root))))
