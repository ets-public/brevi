# filter's copy, deepcopy and pickle follow its __reduce__, (filter, (function, input)): a shallow copy shares
# the input iterator, a deep copy and a pickle round trip do not.
import copy, pickle

def even(x):
    return x % 2 == 0

c = filter(even, iter(range(10)))
next(c)
s = copy.copy(c)
print(next(s), next(c))
d = copy.deepcopy(c)
print(list(d), next(c))
for proto in range(pickle.HIGHEST_PROTOCOL + 1):
    p = pickle.loads(pickle.dumps(filter(even, range(6)), proto))
    print(type(p).__name__, list(p))
print(list(c))
