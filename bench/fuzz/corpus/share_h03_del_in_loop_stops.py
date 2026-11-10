xs = list(range(10))
for i, x in enumerate(xs):
    if x % 2 == 0:
        del xs[i]
print(xs)
