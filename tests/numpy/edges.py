import numpy as np

# dtypes and NEP 50 promotion
a = np.array([1, 2, 3])
print(a.dtype, (a + 1).dtype, (a + 1.5).dtype, (a / 2).dtype, (a > 1).dtype)
print(np.array([True, False]) + 1, np.array([1.0, 2.0]) * True)
print(repr(np.int64(7) + 1), repr(np.float64(2.5) * 2), repr(np.True_))
# printing
print(np.array([1e-5, 1.0, 1e5]))
print(np.array([0.1, 0.25, 1 / 3]))
print(repr(np.array([], dtype=float)), repr(np.zeros((2, 0))))
print(np.arange(2000))
np.set_printoptions(precision=3, suppress=True)
print(np.array([1e-9, 3.14159265, 2e7]))
np.set_printoptions(precision=8, suppress=False)
print(repr(np.array([[1.5, -2.0], [np.nan, np.inf]])))
# views share memory
m = np.arange(12).reshape(3, 4)
v = m[1:, ::2]
v[0, 0] = 99
print(m)
print(m.T[::-1], m[:, 1].sum(), m[m > 5])
# broadcasting and reductions
x = np.linspace(0, 1, 7)
print(x[:, None] * np.arange(3))
print(m.sum(axis=0), m.mean(axis=1), m.std(), m.max(axis=1, keepdims=True))
print(np.cumsum(m, axis=1), np.argmax(m), np.argsort(np.array([3, 1, 2])))
# pairwise summation order
f = np.arange(1000) * 0.1
print(f.sum(), f.reshape(10, 100).sum(axis=1)[:3], f.mean())
# rounding: half to even
print(np.round(np.array([0.5, 1.5, 2.5, -0.5]), 0), np.round(2.675, 2))
# errors numpy raises
for bad in [lambda: np.zeros(3) + np.zeros(4), lambda: m.sum(axis=5), lambda: np.array([[1, 2], [3]]), lambda: bool(np.zeros(2))]:
    try:
        bad()
    except Exception as e:
        print(type(e).__name__, e)
# warnings go to stderr, results stay numpy's
print(np.array([1.0, -1.0, 0.0]) / 0)
print(np.unique(np.array([3, 1, 3, 2, 1]), return_counts=True))
print(np.where(m > 4, m, -1), np.isclose(0.1 + 0.2, 0.3), np.allclose(x, x + 1e-12))
print(m @ m.T, np.dot(np.arange(3), np.arange(3)))
print(np.concatenate([a, a]), np.vstack([a, a]).shape, np.stack([a, a], axis=1).tolist())
