# (x,) + t joins tuples, also when t is held as JSON.
lab = {}
lab[("r1", 0)] = (1, ["r1"])
c = lab[("r1", 0)]
c = (c[0] + 1, c[1] + ["r2"])
lab[("r2", 1)] = c
res = None
for k in lab:
    x = (5,) + lab[k]
    if res is None or x < res:
        res = x
print(res, ",".join(res[2]))
best = None
lab2 = {}
for i, w in enumerate(["a", "b"]):
    best = (1, [w]) if best is None else (best[0] + 1, best[1] + [w])
    lab2[w] = best
for w in lab2:
    print((9,) + lab2[w])
