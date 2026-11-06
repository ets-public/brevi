# matrix @ matrix (OpenBLAS's dgemm; bit-exact against numpy on AVX-512 and AVX2 when checked with repr; printed
# at numpy's precision so the expected file holds on either kind of CPU)
import numpy as np
rng = np.random.default_rng(7)

for m in [2, 3, 5, 9, 13]:
    for k in [1, 2, 16, 17, 40]:
        for n in [2, 4, 5, 9]:
            X = rng.standard_normal((m, k)); Y = rng.standard_normal((k, n))
            Xt = rng.standard_normal((k, m)).T; Yt = rng.standard_normal((n, k)).T
            print(m, k, n, X @ Y, Xt @ Y, X @ Yt, np.dot(Xt, Yt))
# one row, one column, sliced, reversed and Fortran-like operands
for (m, k, n) in [(5, 40, 7), (13, 17, 9), (30, 100, 21)]:
    big = rng.standard_normal((k + 3, m + 4))
    Xf = big[:k, :m].T
    Xr = rng.standard_normal((m, k))[::-1]
    Xc = rng.standard_normal((m, 3 * k))[:, ::3]
    Y = rng.standard_normal((k, n)); Yr = rng.standard_normal((k, n))[:, ::-1]
    for a in (Xf, Xr, Xc):
        for b in (Y, Yr):
            print(np.array([(a @ b).sum()]), np.dot(a, b)[1], (a[:1] @ b)[0][:3], (a @ b[:, :1])[:3].ravel())
# larger: the regular kernels, k split into blocks, several threads
for (m, k, n) in [(40, 1000, 40), (70, 300, 50), (120, 120, 120), (200, 30, 200)]:
    X = rng.standard_normal((m, k)); Y = rng.standard_normal((k, n))
    P = X @ Y
    print(P.shape, P[0][:4], P[-1][-3:], np.array([P.sum()]))
A = rng.standard_normal((6, 4))
print(A @ A.T)
# a matrix times its own transpose (numpy calls dsyrk)
for (m, k) in [(5, 3), (13, 17), (40, 5), (70, 30), (33, 500), (150, 30)]:
    A = rng.standard_normal((m, k))
    G = A @ A.T; H = A.T @ A
    print(G.shape, G[0][:3], G[-1][-3:], np.array([G.sum(), H.sum(), np.dot(A, A.T).sum()]), H[1][:2])
