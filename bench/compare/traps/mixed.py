n = int(input())
items = [1, "two", 3.0, None, [4], (5, 6), {"k": 7}]
for x in items[:n]:
    print(type(x).__name__, x)
v = 1
v = "now a string"
print(v)
print(sorted([3, 1, 2]) + ["a"], max("abc", "b"), [1, 2] * 2, "ab" * 3)
