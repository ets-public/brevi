from itertools import accumulate, chain, count, groupby, islice, zip_longest, cycle, starmap, takewhile, dropwhile, pairwise, compress, filterfalse
print(list(islice(chain(count(), "ab"), 3)))
a=[]; c=chain(a); a.append(5); print(list(c))
print(list(chain.from_iterable([[1,2],"xy",()])))
print(list(accumulate([1,2,3,4])), list(accumulate([1,2,3], initial=10)), list(accumulate([3,1,4], max)))
print(list(islice(accumulate(count()), 5)))
for k, g in groupby("aaabbcda"): print(k, list(g))
gs = list(groupby("aabbc")); print([(k, list(g)) for k, g in gs])
g = groupby("aabb"); k, gr = next(g); print(next(gr)); k2, gr2 = next(g); print(k2, list(gr), list(gr2))
print([(k, len(list(g))) for k, g in groupby([1,1,2,3,3,3], key=lambda x: x % 2)])
print(list(zip_longest("abc", [1], fillvalue="-")), list(zip_longest()), list(zip_longest("ab", count())) if False else 0)
print(list(islice(cycle("ab"), 5)), list(cycle([])))
print(list(starmap(pow, [(2,3),(3,2)])))
print(list(takewhile(lambda x: x<3, count())), list(dropwhile(lambda x: x<3, [1,4,1,5])))
print(list(pairwise("abcd")), list(pairwise("a")), list(pairwise([])))
print(list(compress("abcdef", [1,0,1,0,1,1])), list(compress(count(), [0,1,1])))
print(list(filterfalse(None, [0,1,"",2])), list(filterfalse(lambda x: x%2, range(6))))
try:
    chain(1)
    print("lazy ok")
    list(chain(1))
except TypeError as e: print("TE", e)
try: accumulate(5)
except TypeError as e: print("TE", e)

# re-entrant inputs, as CPython's C iterators allow
class I:
    count = 0
    def __iter__(self):
        return self
    def __next__(self):
        self.count += 1
        if self.count == 2:
            return next(it)
        return [self.count]
it = pairwise(I())
print(next(it), next(it))

def key(n):
    if n == 5:
        list(grp)
    return n != 6
for k, grp in groupby(range(10), key):
    print(k, list(grp))

def mutating():
    first = [1]
    def g(v):
        if first:
            del first[:]
            print("inner", next(z))
        return v
    gen = map(g, [1, 4, 5, 6, 2, 3])
    z = zip(*[gen] * 3)
    print("outer", next(z))
mutating()

class N:
    def __iter__(self):
        return self
try:
    filter(None, N())
except TypeError as e:
    print("TE", e)
print(list(zip("ab", "cd", strict=True)))
try:
    list(zip("ab", "c", strict=True))
except ValueError as e:
    print("VE", e)
try:
    list(zip("a", "cd", strict=True))
except ValueError as e:
    print("VE", e)
