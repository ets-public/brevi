# Loops over sets whose passes do not depend on order: they may be typed.
jobs = {"a": (3, ["b", "c", "b"]), "b": (2, []), "c": (1, ["b"])}
indeg = {n: 0 for n in jobs}
children = {n: [] for n in jobs}
for n, (dur, deps) in jobs.items():
    for d in set(deps):
        indeg[n] += 1
        children[d].append(n)
print(indeg, {k: sorted(v) for k, v in children.items()})
reach = {0, 3, 5, 9}
nxt = set()
for q in reach:
    if q < 8 and q not in nxt:
        nxt.add(q + 1)
print(sorted(nxt), len(nxt))
