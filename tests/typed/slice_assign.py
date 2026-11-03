xs = list(range(10))
ys = [0] * 8
xs[2:5] = [100, 200]
print(xs, ys)
xs[-2:] = [7, 8, 9]
print(xs)
xs[5:3] = [1]
print(xs)
xs[:3] = xs[2::-1]
print(xs)
xs[8:] = []
print(xs, len(ys))
xs[-100:2] = [0]
print(xs)
