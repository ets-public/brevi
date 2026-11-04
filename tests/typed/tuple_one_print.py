# One-item tuples kept apart from real tuples: this program stays typed (no one-item tuple is shown whole).
best = {}
v = (0, ("s",), "x")
c = (v[0] + 1, v[1] + ("u",), "via")
best[1] = c
ans = best[1]
print("%s %d" % (ans[2], len(ans[1])))
d = {("a",): 1}
print(d[("a",)], "%s" % ("x",))
n = 3
print("%d items" % (n,))
print("[%s]" % ("a",))
