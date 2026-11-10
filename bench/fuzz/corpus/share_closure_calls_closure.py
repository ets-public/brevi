def evaluate(toks):
    pos = [0]
    seen = pos
    def eat():
        t = toks[pos[0]]
        pos[0] += 1
        return t
    def two():
        return eat() + eat()
    r = two()
    print(r, pos, seen)
evaluate([1, 2, 3])
def outer():
    log = []
    keep = log
    def add(x):
        log.append(x)
    def add_twice(x):
        add(x)
        add(x)
    add_twice(5)
    print(keep)
outer()
