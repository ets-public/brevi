# sum() of floats is compensated (Neumaier) since CPython 3.12, in every form the typed build takes.
import json
import math


def total(xs):
    return sum(xs)


def scale(xs, k):
    return sum(x * k for x in xs)


tenths = [0.1] * 10
big = [1e100, 1.0, -1e100]

# A list, literal or held in a name.
print(sum([0.1] * 10))
print(sum([1e100, 1.0, -1e100]))
print(sum(tenths))
print(sum(big))
print(total(tenths))

# Generators and comprehensions.
print(sum(x for x in tenths))
print(sum([x for x in big]))
print(sum(x * 1.0 for x in big if x != 0.0))
print(sum(0.1 for _ in range(10)))
print(scale(tenths, 1.0))
print(sum(i * 0.1 for i in range(1, 11)))

# A start value is the first term of the compensated sum.
print(sum(tenths, 0.0))
print(sum(big, 0.0))
print(sum([1.0, -1e100], 1e100))
print(sum(tenths, 1.0))
print(sum((x for x in [1.0, -1e100]), 1e100))
print(sum(tenths, 1))
print(1e100 + sum([1.0, -1e100]))
print(sum(x for x in [1.0, -1e100]) + 1e100)

# Other iterables of floats.
print(sum((0.1, 0.2, 0.3)))
print(sum({"a": 0.1, "b": 0.2, "c": 0.3}.values()))
print(sum(map(float, ["0.1", "0.2", "0.3"])))
print(sum(tenths[:5]))
print(list(map(sum, [tenths, big])))

# Numbers read from JSON.
data = json.loads('{"xs": [0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1], "big": [1e100, 1.0, -1e100], "ints": [1, 2, 3]}')
print(sum(data["xs"]))
print(sum(data["big"]))
print(sum(data["ints"]))
print(sum(x for x in data["xs"]))

# Edge values: signs, overflow, infinities, nan.
print(sum([-0.0]))
print(sum([-0.0, -0.0]))
print(sum([1e308, 1e308]))
print(sum([1e308, 1e308, -1e308]))
print(sum([math.inf, 1.0]))
print(sum([math.inf, -math.inf]))
print(sum([math.nan, 1.0]))
