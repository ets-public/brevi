# np.linalg.lstsq, np.polyfit and np.polyval: LAPACK dgelsd as numpy's OpenBLAS runs it, to the last digit
# (full precision: AVX2 CPUs check lstsq.avx2.expected, numpy's output with OPENBLAS_CORETYPE=Haswell)
import numpy as np

rng = np.random.default_rng(7)
x = np.linspace(0, 10, 50)
y = 3.0 * x**2 - 2.0 * x + 1.0 + np.sin(x)
c = np.polyfit(x, y, 2)
print(c, repr(c))
print(c.tolist())
slope, intercept = np.polyfit(x, y, 1)
print(f"slope {slope:.6f} intercept {intercept:.6f}", slope, intercept)
print(np.polyval(c, 2.5), np.polyval(c, x[:5]).tolist(), np.polyval([1, 2, 3], 2), np.polyval(np.array([1.0, 2]), [1, 2]))

# full output, weights, covariance, several right-hand sides, more coefficients than points
print(np.polyfit(x, y, 3, full=True))
print(np.polyfit(x, y, 3, rcond=1e-3, full=True)[2:])
cc, cov = np.polyfit(x, y, 2, cov=True)
print(cc.tolist(), cov.tolist())
print(np.polyfit(x, y, 2, cov="unscaled")[1].tolist())
print(np.polyfit(x, y, 2, w=np.arange(50.0) + 1).tolist())
print(np.polyfit(x, np.vstack([y, 2 * y + 1]).T, 1).tolist())
print(np.polyfit([1, 2, 3], [2, 4, 7], 5).tolist())
print(np.polyfit(np.arange(5), np.arange(5) ** 2, 0).tolist())

# lstsq: tall, square, wide, rank-deficient, integer input, matrix right-hand side, rcond choices
A = np.vstack([x, np.ones(len(x))]).T
sol, res, rank, sv = np.linalg.lstsq(A, y, rcond=None)
print(sol.tolist(), res.tolist(), rank, sv.tolist(), rank == 2, int(rank), repr(rank))
print(np.linalg.lstsq(np.array([[1.0, 2], [3, 4], [5, 6]]), np.array([1.0, 2, 4])))
print(np.linalg.lstsq(np.arange(12.0).reshape(4, 3), np.arange(8.0).reshape(4, 2)))
print(np.linalg.lstsq(np.arange(12.0).reshape(3, 4), np.array([1.0, 0, 2])))
print(np.linalg.lstsq(rng.integers(-5, 6, (6, 3)), rng.integers(-9, 10, 6)))
for m, n in [(30, 4), (7, 7), (5, 9), (4, 20), (200, 12), (2000, 3)]:
    M = rng.standard_normal((m, n))
    b = rng.standard_normal((m, 2))
    for rc in [None, -1, 1e-10, 0.5]:
        r = np.linalg.lstsq(M, b, rcond=rc)
        print(m, n, rc, r[0].tolist(), r[1].tolist(), int(r[2]), r[3].tolist())
big = rng.standard_normal((20000, 5))
print(np.linalg.lstsq(big, big @ np.arange(1.0, 6.0) + rng.standard_normal(20000), rcond=None)[0].tolist())
print(np.linalg.lstsq(np.ones((4, 2)), np.arange(4.0)))
print(np.linalg.lstsq(np.zeros((3, 2)), np.ones(3)))
print(np.linalg.lstsq(rng.standard_normal((6, 3)) * 1e-300, rng.standard_normal(6))[0].tolist())
print(np.linalg.lstsq(rng.standard_normal((6, 3)) * 1e300, rng.standard_normal(6) * 1e-300)[0].tolist())
try:
    np.linalg.lstsq(np.ones((3, 2)), np.ones(4))
except np.linalg.LinAlgError as e:
    print("LinAlgError:", e)
try:
    np.polyfit([1, 2], [1, 2, 3], 1)
except TypeError as e:
    print("TypeError:", e)
