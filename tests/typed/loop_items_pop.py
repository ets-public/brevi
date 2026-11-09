# `for k, v in d.items(): v.pop(0)`: the pop ends the loop body, and the change must still reach d (the typed build
# left it in a copy). Found by the copy-versus-alias sweep after the DeepSeek release test, 9 Nov 2026.
d = {"a": [1], "b": [2]}
for v in d.values():
    v.append(0)
for k, v in d.items():
    v.pop(0)
print(d)
