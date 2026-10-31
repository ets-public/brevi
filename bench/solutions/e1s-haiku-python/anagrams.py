import sys

words = set(w.lower() for w in sys.stdin.read().split())
groups = {}
for w in words:
    key = "".join(sorted(w))
    groups.setdefault(key, []).append(w)

result = []
for key, ws in groups.items():
    if len(ws) >= 2:
        result.append(sorted(ws))

result.sort(key=lambda g: (-len(g), g[0]))
out = [" ".join(g) for g in result]
if out:
    print("\n".join(out))
