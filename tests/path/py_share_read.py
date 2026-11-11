# A function from a sibling module that only reads the dict it is given: the typed build, even though
# the dict is changed after the call while the function's answer is still used.
from _share_read import top

counts = {"a": 3, "b": 1, "c": 2}
best = top(counts, 2)
counts["d"] = 9
for w, c in best:
    print(w, c)
print(sorted(counts))
