xs = [1]
try:
    xs.push(2)
except AttributeError as e:
    print("no push")
if hasattr(xs, "first"):
    print(xs.first())
print(xs, 1 != 2, f"{1!r}", 5 & 3, 5 | 3)
