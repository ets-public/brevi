# np.linalg.norm/matrix_power, histogram, cov, corrcoef, trapezoid and ufunc reduce/accumulate.
# Floats from dot products are rounded: their last digit depends on the CPU's BLAS kernel, as in numpy.
import numpy as np

g = np.random.default_rng(3)
v = g.uniform(-3, 3, 7)
M = g.uniform(-2, 2, (4, 4))
print(np.linalg.norm(v).round(10), np.linalg.norm(M).round(10), np.linalg.norm(v, 1), np.linalg.norm(v, np.inf))
print(np.linalg.norm(v, 3).round(10), np.linalg.norm(M, axis=1).round(10), np.linalg.norm(M, 1), np.linalg.norm(M, np.inf))
print(np.linalg.norm(np.array([3, 4])), np.linalg.norm([[1, 2], [3, 4]], axis=0, keepdims=True).round(10))
print(np.linalg.matrix_power(np.array([[1, 1], [1, 0]]), 10), np.linalg.matrix_power(M, 3).round(6), np.linalg.matrix_power(M, 0))
data = g.normal(0, 1, 1000)
h, e = np.histogram(data, bins=12)
print(h, e)
print(np.histogram([1, 2, 2, 3, 3, 3], bins=3), np.histogram(np.arange(10), bins=[0, 2, 5, 9]), np.histogram([5, 5, 5]))
print(np.histogram(data, bins=5, range=(-2, 2), density=True))
x = g.uniform(0, 10, 20)
y = 2 * x + g.normal(0, 1, 20)
print(np.cov(x, y).round(10))
print(np.corrcoef(x, y).round(10))
print(round(float(np.corrcoef(x, y)[0, 1]), 10), repr(np.cov([1, 2, 3, 4])), np.cov(np.vstack([x, y, x * y])).round(8))
print(np.trapezoid([1, 2, 3]), np.trapezoid(np.sin(np.linspace(0, np.pi, 50)), np.linspace(0, np.pi, 50)), np.trapezoid([1, 4, 9], dx=0.5))
a = np.array([3, 1, 4, 1, 5, 9, 2, 6])
f = g.normal(0, 1, 8)
print(np.maximum.accumulate(a), np.minimum.accumulate(f), np.add.accumulate(a), np.multiply.reduce(a), np.add.reduce(f))
print(np.maximum.reduce(np.arange(12).reshape(3, 4), axis=1), np.maximum.accumulate(np.arange(12).reshape(3, 4)[::-1]))
print(np.logical_or.reduce([False, True]), np.logical_and.reduce(np.array([[True, False], [True, True]])))
try:
    np.linalg.norm(M, 'bogus')
except ValueError as err:
    print("ValueError", err)
