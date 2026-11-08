# A package of the program's own (from pkg.module import ...) builds with it; random's state is shared with
# the package (it used to go to CPython, which gave the package its own unseeded random).
import random
from tinygrad_pkg.engine import Value, weight

random.seed(7)
ws = [weight() for _ in range(3)]
xs = [[1.0, -2.0, 0.5], [0.3, 0.1, -1.0], [2.0, 2.0, 2.0]]
ys = [1.0, -1.0, 1.0]
for step in range(20):
    loss = sum(((sum((w * x for w, x in zip(ws, row)), Value(0.0)) - y) ** 2 for row, y in zip(xs, ys)), Value(0.0))
    for w in ws:
        w.grad = 0
    loss.backward()
    for w in ws:
        w.data -= 0.05 * w.grad
    if step % 5 == 0:
        print(step, round(loss.data, 6))
print([round(w.data, 6) for w in ws], random.random())
