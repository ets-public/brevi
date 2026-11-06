# matrix @ vector and vector @ matrix (OpenBLAS's dgemv; bit-exact against numpy on AVX-512 and AVX2 when checked
# with repr; printed at numpy's precision so the expected file holds on either kind of CPU)
import numpy as np
rng = np.random.default_rng(3)


out = []
for k in list(range(1, 30)) + [63, 64, 65, 200]:
    for m in [1, 2, 3, 4, 5, 6, 7, 9, 13]:
        A = rng.standard_normal((m, k)); x = rng.standard_normal(k); B = rng.standard_normal((k, m)); C = rng.standard_normal((k, 3))
        for nm, v in (("Ax", A @ x), ("xB", x @ B), ("dot", np.dot(A, x))):
            out.append(nm + " " + str(v))
print("\n".join(out))
# transposed (Fortran-ordered) and sliced operands, strided vectors, np.dot
for (m, k) in [(4, 9), (7, 13), (3, 10), (2, 7), (9, 22)]:
    A = rng.standard_normal((m, 2 * k)); x = rng.standard_normal(2 * k); z = rng.standard_normal(2 * m)
    for v in (A[:, ::2] @ x[::2], z[::2] @ A, A[::2] @ x, np.dot(A, x), A.T @ z[:m], x @ A.T, np.dot(A[:, ::2], x[::2])):
        print(v)
