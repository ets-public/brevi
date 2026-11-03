# random runs without allow native (Brevi's own copy of CPython's generator): seeded draws match CPython.
import random
random.seed(1)
print(random.randint(1, 6), random.random(), random.choice("abcdef"))
xs = list(range(10))
random.shuffle(xs)
print(xs, random.sample(range(50), 5))
r = random.Random(42)
print([r.randrange(100) for _ in range(5)], round(r.gauss(0, 1), 12))
