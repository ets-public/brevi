import heapq

# merge only combines the current heads of its inputs: it never sorts them
print(list(heapq.merge([3, 1, 2], [0, 5, 4])))

# equal values: the earlier input comes first, then that input's next value is tried
print(list(heapq.merge([1, 2], [1, 2])))
print(list(heapq.merge([2, 1], [1, 2])))

# empty inputs
print(list(heapq.merge([], [3, 2])))
print(list(heapq.merge([3, 2], [])))
print(list(heapq.merge()))

# reverse: largest current head first, ties still by input order
print(list(heapq.merge([3, 1, 2], [0, 5, 4], reverse=True)))
print(list(heapq.merge([1, 2, 3], [2, 1], reverse=True)))

# key is applied per pulled value while more than one input is live
print(list(heapq.merge([1, 2], [3, 4], key=lambda x: -x)))
print(list(heapq.merge([1, 2], [3, 4], key=lambda x: -x, reverse=True)))
print(list(heapq.merge([(1, 'a'), (2, 'b')], [(1, 'c')], key=lambda p: p[0])))
print(list(heapq.merge([(1, 'a'), (2, 'b')], [(1, 'c')], key=lambda p: p[0], reverse=True)))

# as in CPython, key stops being applied once only one input remains
def keyerr(x):
    if x == 3:
        raise KeyError("badkey")
    return -x
print(list(heapq.merge([1, 3], [2, 4], key=keyerr)))

# lazy: the first item of every input is pulled up front, the rest as they are used
def gen(n):
    print("pull", n)
    yield n
    yield n + 10
it = heapq.merge(gen(1), gen(2))
print(next(it))
print(next(it))
print(next(it))

try:
    print(list(heapq.merge(1)))
except TypeError as e:
    print("TypeError", e)
try:
    heapq.merge(foo=1)
except TypeError as e:
    print("TypeError", e)
