# float32 and int32 arrays: NEP 50 promotion, float32 rounding, int32 wrapping, float32 printing
import numpy as np

a = np.array([1.5, 2.25, -3.1, 1e-5, 7e6], dtype=np.float32)
b = np.arange(5, dtype=np.int32)
print(a, a.dtype)
print(repr(a))
print(b, repr(b), b.dtype)
print(a + 1, a * 2.5, a / 3, a // 2, a % 2)
print(a + b, (a + b).dtype)
print(b + 1, (b + 1).dtype, b * 3.5, (b * 3.5).dtype)
print(b + np.arange(5), (b + np.arange(5)).dtype, (a + np.zeros(5)).dtype)
print(a.sum(), a.mean(), a.var(), a.std(), a.prod(), a.max(), a.argmin())
print(b.sum(), b.mean(), b.cumsum(), b.prod())
print(np.floor(a), a ** 2, a ** -1, np.sqrt(abs(a)), np.square(a), np.reciprocal(a))

# float32 and int32 scalars
s = np.float32(0.1)
print(s, repr(s), s + 0.2, repr(s * 3), s == 0.1, repr(s + np.float64(0.2)))
i = np.int32(7)
print(i, repr(i), i // 2, i + 1, repr(i * 2), repr(i + 2.5), i < 2**40)
print(np.float32(1e6), np.float32(123456.7), np.float32(1e-4), np.float32(3.0), np.float32(1e-5), np.float32(2333333.25))
try:
    print(i + 2**40)
except OverflowError as e:
    print("OverflowError:", e)

# int32 wraps around
big = np.array([2**31 - 1, -2**31], dtype=np.int32)
print(big + 1, big * 2, -big)

# dtype= in constructors, astype casts
x = np.linspace(0, 1, 7, dtype=np.float32)
print(x, x.sum())
print(np.arange(0, 2.5, 0.5, dtype=np.int64), np.arange(0, 1, 0.1, dtype=np.float32), np.arange(7, dtype='i4'))
c = np.array([1e40, -1.0, 3.9])
print(c.astype(np.float32), c.astype(np.int32))
print(np.zeros(3, dtype=np.float32), np.ones((2, 2), dtype='int32'), np.full(3, 0.1, dtype='f4'))

# reductions and sorting on bigger arrays
r = np.random.default_rng(0).random(1000).astype(np.float32)
print(r.sum(), r.mean(), r.var(), r.std(), r.min(), r.max())
print(np.round(r[:5], 3), r[:5].astype(np.int32), r[:3].tolist())
m = (r * 100).astype(np.int32)
print(m[:10], m.sum(), m.mean(), m.max(), np.sort(m[:10]), np.argsort(m[:6], kind="stable"))
print(m[m > 50][:5], np.where(m > 50, m, 0)[:6], np.unique(m[:20]))
print(np.maximum(a, 2), np.minimum(b, 2), np.abs(a), np.clip(a, -1, 1), np.cumsum(a))
print(np.concatenate([a, b]).dtype, np.concatenate([b, b]).dtype)
print(np.array([[1, 2], [3, 4]], dtype=np.float32).T, np.array([[1, 2], [3, 4]], dtype=np.int32).reshape(4))
print(repr(np.arange(1001, dtype=np.int32)))
print(np.array([0.0001, 5.0], dtype=np.float32), np.array([999999.0, 2.5], dtype=np.float32), np.array([1e6, 2.5], dtype=np.float32))
