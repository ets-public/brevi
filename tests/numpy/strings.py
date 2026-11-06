# String arrays and np.str_ scalars in the native numpy, checked against numpy 2
import numpy as np
a = np.array(['Widget', 'Gadget', 'Doohickey', 'a'])
print(a); print(repr(a)); print(a.dtype, a.itemsize, a.shape)
print(a[0], repr(a[0]))
print(a == 'Widget', a < 'H', np.sort(a), np.unique(a), np.argsort(a), a[[1, 0]], a[a != 'a'])
b = a.copy(); b[0] = 'VeryLongName12'; print(b)
print(np.where(a == 'a', 'X', a), np.where(a == 'a', 'longer than all', a).dtype)
print(np.concatenate([a, np.array(['zz'])]).dtype, list(a), a.tolist(), str(a[0]) + '!', a[0] + '!')
print(a + '_x', (a + '_x').dtype, 'x' + a, a + a)
print(np.array([1, 'a']), np.array(['1.5', '2']).astype(float), np.arange(3).astype(str), np.array([1.5, 2.25]).astype(str))
print(np.array([['a', 'bb'], ['ccc', 'd']]))
print(len(a[0]), a[0].upper(), f'{a[0]:>10}|', {a[0]: 1}, 'Widget' in a)
print(np.full(3, 'ab'), np.zeros(2, dtype='U3'), np.empty(2, dtype=str))
print(repr(np.array([], dtype=str)), repr(np.array(['a'], dtype='U5')), repr(np.array(["a'b", 'c"d'])))
print(repr(np.array(['x' * 30] * 4)))
print(np.argmax(np.array(['b', 'a', 'c'])), np.array(['a', '']).any(), repr(np.dtype('<U3')), a.dtype.kind)
for name in a[:2]:
    print(name, type(name).__name__)
d = dict(zip(a, range(4)))
print(d, d['Gadget'], sorted(a), ",".join(a), isinstance(a[0], str), a[0] == 'Widget', 'Wid' in a[0])
import json
import re
from collections import Counter
names = np.array(["alice", "bob", "carol", "bob", "12"])
x = names[0]
print(x, repr(x), [x], (x,), {x: 1}, f"{x:>10}|", x.upper(), x + "!", "!" + x, len(x), "li" in x, x == "alice", x != "bob")
d = {"alice": 5}
print(d[x], d.get(x), x in d, {x, "alice"}, len({x, "alice"}), sorted(names), max(names), min(names))
print(",".join(names), " ".join([x, "z"]), x.split("i"), x.startswith("al"), x.replace("a", "A"), x[1:3], x[0], x * 2, "%s!" % x, "{}?".format(x))
print(json.dumps([str(x)]), int(names[4]) + 1, float(names[4]), str(x), bool(x), hash(x) == hash("alice"), x < "b", x > "a")
c = Counter(names)
print(c.most_common(1), c["bob"], list(c.keys()))
print(re.match(r"a\w+", x).group(0), re.sub("l", "L", x), x.title(), x.center(9, "*"), x.encode() if False else "")
for i, n in enumerate(names):
    if n == "bob":
        print(i, n, type(n).__name__, isinstance(n, str))
print([n.capitalize() for n in names], {n: len(n) for n in names})
print(names.tolist(), names[names != "bob"], np.unique(names, return_counts=True))
print(sum(len(n) for n in names), sorted(names, key=len), list(map(str.upper, names)))
m = np.arange(24).reshape(2, 3, 4)
s = m.swapaxes(0, 2); print(s.shape, s[1, 2, 0])
s[0, 0, 0] = 99; print(m[0, 0, 0])
print(np.swapaxes(m, 1, -1).shape)
w = np.array([['a', 'bb'], ['c', 'd']]); print(w.swapaxes(0, 1))
print(np.str_('abc'), repr(np.str_('x')), np.str_('ab') + 'c', np.str_(), repr(np.str_(1.5)), np.str_(np.float64(2.0)))
print(np.str_(np.arange(3)), np.str_('ab') == 'ab', isinstance(np.str_('q'), str))
print(np.array(['a', 'b'], dtype=np.str_), np.zeros(2, dtype=np.str_))
# what numpy has no loop for raises numpy's own errors
for f in (lambda: a * 2, lambda: a - 'x', lambda: np.sum(a), lambda: a.mean(), lambda: np.sqrt(a), lambda: np.array(['x', '1']).astype(int), lambda: np.maximum(a, a)):
    try:
        print(f())
    except Exception as e:
        print(type(e).__name__, e)
print(a == 3, a != 3, np.array(['10', '2']).astype(np.int32), np.array(['3']).astype(np.float32))
v = a[1:3]; v[0] = 'CHANGED'; print(a, a.dtype)
a[a == 'a'] = 'A!'; a[3] = 5; print(a)
big = np.array(['w%d' % i for i in range(1200)]); print(big, big[::7][:5], np.unique(big[:50] < 'w3').tolist())
