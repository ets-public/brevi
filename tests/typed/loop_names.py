# Names set in a loop (or before break) are read after it; for/else; `x or []`; any() with a changing argument;
# "\x01%d" escapes.
def look(xs, k):
    for el in reversed(xs):
        if k in el:
            v = el
            break
    else:
        return "none"
    return v[k]
print(look([{"a": "1"}, {"b": "2"}], "a"), look([{"a": "1"}], "c"))
rec = {"a": ["b", "c"], "b": ["c"], "c": []}
def reach(a, target, seen):
    if a == target:
        return True
    if a in seen:
        return False
    seen.add(a)
    return any(reach(i, target, seen) for i in rec[a])
print(reach("a", "c", set()), reach("c", "a", set()))
def opts(t):
    deps = None
    prio = None
    for o in t:
        if o.startswith('a=') and deps is None:
            deps = o[2:].split(',')
        elif o.startswith('p=') and prio is None:
            prio = int(o[2:])
    return (len(t), deps or [], prio or 0)
print(opts(["x", "a=b,c", "p=3"]), opts(["y"]))
n = 3
print(repr("\x01%d\x02" % n), repr("a\x00b%s" % "z"))
