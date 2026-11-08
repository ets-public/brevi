# Trains micrograd's MLP on a two-moons dataset, as Karpathy's demo notebook does, with the package unchanged.
# The data is made here (no sklearn or numpy) so the run is the same on both sides.
import math
import random

from micrograd.engine import Value
from micrograd.nn import MLP

random.seed(1337)


def moons(n, noise=0.1):
    xs, ys = [], []
    for i in range(n):
        t = math.pi * (i // 2) / (n // 2 - 1)
        if i % 2 == 0:
            x, y, label = math.cos(t), math.sin(t), -1
        else:
            x, y, label = 1 - math.cos(t), 0.5 - math.sin(t), 1
        xs.append([x + random.gauss(0, noise), y + random.gauss(0, noise)])
        ys.append(label)
    return xs, ys


X, y = moons(100)
model = MLP(2, [16, 16, 1])
print(model)
print("number of parameters", len(model.parameters()))


def loss():
    scores = [model([Value(a) for a in row]) for row in X]
    # svm max-margin loss, plus L2 regularization
    losses = [(1 + -yi * si).relu() for yi, si in zip(y, scores)]
    data_loss = sum(losses) * (1.0 / len(losses))
    reg_loss = 1e-4 * sum(p * p for p in model.parameters())
    accuracy = [(yi > 0) == (si.data > 0) for yi, si in zip(y, scores)]
    return data_loss + reg_loss, sum(accuracy) / len(accuracy)


for k in range(30):
    total, acc = loss()
    model.zero_grad()
    total.backward()
    lr = 1.0 - 0.9 * k / 100
    for p in model.parameters():
        p.data -= lr * p.grad
    print("step %d loss %.6f, accuracy %.1f%%" % (k, total.data, acc * 100))
