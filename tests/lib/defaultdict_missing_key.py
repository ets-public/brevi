# reading a missing key of a defaultdict inserts it with the default (CPython __missing__)
from collections import defaultdict

counts = defaultdict(int)
for w in "a b a c b a".split():
    counts[w] += 1
print(dict(counts), len(counts))
print(counts["zzz"], len(counts), "zzz" in counts, list(counts))
print(counts.get("nope"), "nope" in counts, len(counts))

groups = defaultdict(list)
groups["x"].append(1)
g = groups["y"]
print(sorted(groups.items()), g)

totals = defaultdict(float)
totals["p"] += 2.5
print(totals["q"], sorted(totals))

names = defaultdict(str)
names["k"] += "ab"
print(repr(names["m"]), sorted(names.items()))

