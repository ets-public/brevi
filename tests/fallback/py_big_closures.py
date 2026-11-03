def outer(n):
    f = 1
    def show():
        return f + 1
    for i in range(1, n):
        f *= i
    return show()
def g():
    total = 0
    def add(k):
        nonlocal total
        total = total * 3 + k
    for i in range(50):
        add(i)
    return total
print(outer(10), outer(25), g())
total = 1
def bump(k):
    global total
    total = total * 7 + k
for i in range(40):
    bump(i)
print(total)
def fn_in_main():
    acc = 1
    def mul(v):
        nonlocal acc
        acc *= v
    for v in range(2, 30):
        mul(v)
    return acc
print(fn_in_main())
