# `lst = d.get(k) or []`: lst is the stored list only when that list is not empty, so changing it
# cannot be written back; the typed build changed a copy. Found by the copy-versus-alias sweep (9 Nov 2026).
d = {"a": [3]}
lst = d.get("a") or []
lst.append(4)
m = d.get("zz") or []
m.append(1)
print(d)
