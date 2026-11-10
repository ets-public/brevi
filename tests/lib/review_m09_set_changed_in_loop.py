# A set mutated while it is being iterated: CPython's iterator checks the entry count on each next() and
# raises RuntimeError once it changed; it walks the live table, so a remove + add that keeps the count the
# same is allowed, and once it has failed it stays failed.
ss = {1, 2}
try:
    for v in ss:
        ss.add(v + 10)
    print("no error", sorted(ss))
except RuntimeError as e:
    print("RuntimeError", e)

# add of an already-present value / discard of an absent one: the count is the same, no error
s = {1, 2, 3}
for v in s:
    s.add(v)
    s.discard(v + 100)
print("same size", sorted(s))

# remove + add in one body: count unchanged, iteration walks the live table
s = {1, 2, 3}
seen = []
for v in s:
    seen.append(v)
    s.discard(v)
    s.add(v * 10)
print("replace", seen, sorted(s))

# size changed before the first next()
s = {1, 2, 3}
it = iter(s)
s.add(4)
try:
    next(it)
except RuntimeError as e:
    print("RuntimeError", e)

# once it has raised, the same iterator keeps raising even if the size is restored
s = {1, 2, 3, 4, 5}
it = iter(s)
first = next(it)
s.remove(first)
out = []
for _ in range(3):
    try:
        out.append(next(it))
    except RuntimeError as e:
        out.append("err")
        s.add(100)
print("sticky", out, sorted(s))
