import numpy as np
def mk(r, c, s):
    return (np.sin(np.arange(r * c) * 0.37 + s) * 3.0).reshape(r, c).astype(np.float32)
for m, k, n in [(2, 3, 4), (16, 16, 16), (33, 17, 65), (64, 64, 64), (128, 129, 130), (300, 300, 300), (9, 1500, 9), (4, 64, 128)]:
    a = mk(m, k, 0.1)
    b = mk(k, n, 0.7)
    r1 = a @ b
    r2 = np.dot(a, b)
    print(m, k, n, r1.ravel()[:2].tolist(), r1.ravel()[-2:].tolist(), r1.astype(np.float64).sum(), r2.astype(np.float64).sum())
for L in (1, 7, 31, 32, 33, 100, 1000, 10000, 10001, 50000):
    x = np.sin(np.arange(L) * 0.11).astype(np.float32)
    y = np.cos(np.arange(L) * 0.23 + 1.0).astype(np.float32)
    print("sdot", L, np.dot(x, y), x @ y, np.dot(x[::2], y[::2]))
