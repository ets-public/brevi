import random
from random import Random, randint

random.seed(12345)
print(random.random(), randint(1, 6), random.randrange(10), random.randrange(5, 50, 5), random.randrange(10, 0, -3))
print(random.getrandbits(1), random.getrandbits(32), random.getrandbits(33), random.getrandbits(63), random.getrandbits(64), random.getrandbits(100))
print(random.randrange(2 ** 70), random.randint(-10 ** 20, 10 ** 20))
xs = list(range(20))
random.shuffle(xs)
print(xs, random.choice("abcdef"), random.choice([1.5, 2.5]))
print(random.sample(range(100), 5), random.sample(["a", "b", "c", "d"], 4), random.sample(range(10 ** 6), 3))
print(random.choices("xyz", k=5), random.choices([1, 2, 3], weights=[1, 0, 5], k=6), random.choices("ab", cum_weights=[1, 3], k=4))
print(random.uniform(1, 3), round(random.gauss(10, 2), 12), round(random.gauss(0, 1), 12))
r = Random(1208925819614629174706183)
r2 = Random(-5)
print(r.random(), r.randint(0, 9), r2.random(), Random(0).random(), Random(4294967296).random())
random.seed(3)
a = [random.random() for _ in range(3)]
random.seed(3)
print(a == [random.random() for _ in range(3)])
t = (1, 2, 3)
try:
    random.choice([])
except IndexError as e:
    print("IndexError:", e)
try:
    random.randrange(0)
except ValueError as e:
    print("ValueError:", e)
total = 0
for i in range(100000):
    total += random.randint(1, 100)
print(total)
