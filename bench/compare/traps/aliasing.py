n = int(input())
a = [[0] * 3] * n
a[0][1] = 5
print(a)
b = a[0]
b.append(9)
print(a[1], len(a[n - 1]))
def add(x, acc=[]):
    acc.append(x)
    return acc
add(1); print(add(2))
c = list(range(5)); d = c; d += [5]; print(c)
