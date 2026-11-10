import yaml

def g():
    yield 1
    yield 2

xs = {3, -1, 5}
print(xs)
print(list(g()))
print(yaml.safe_load("a: 1"))
