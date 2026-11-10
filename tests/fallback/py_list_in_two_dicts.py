# One list put in two dicts, then changed: both dicts hold that list in Python; the typed build gave
# each a copy. Found by the copy-versus-alias sweep (9 Nov 2026).
a = []
d1 = {"k": a}
d2 = {"k": a}
a.append(1)
d1["k"].append(2)
print(d1, d2, a)
