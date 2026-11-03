n = int(input())
d = {}
for i in range(n, 0, -1):
    d[str(i * 7 % 13)] = i
del d[str(n * 7 % 13)]
d["zz"] = 1
print(list(d.keys()))
print(d)
s = {3, 1, 2}
print(sorted(s), len(s))
print({k: v for k, v in sorted(d.items())[:3]})
