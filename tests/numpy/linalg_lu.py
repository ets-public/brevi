# np.linalg.det, slogdet, solve and inv: Brevi follows OpenBLAS's own LU, so these match numpy to the last
# digit on AVX2 and AVX-512 CPUs. Random cases are rounded here because the last digit depends on the CPU.
import numpy as np

A = np.array([[3.0, 2, -1], [2, -2, 4], [-1, 0.5, -1]])
b = np.array([1.0, -2, 0])
print(np.linalg.solve(A, b), np.linalg.det(np.array([[1, 2], [3, 4]])), np.linalg.det(np.eye(3) * 2))
print(np.linalg.inv(np.array([[4, 7], [2, 6]])))
print(np.linalg.solve([[2.0, 0.0], [0.0, 4.0]], [[1.0, 3.0], [2.0, 8.0]]))
sign, logdet = np.linalg.slogdet(np.array([[-2.0, 1], [1, 3]]))
print(sign, round(float(logdet), 10), np.linalg.slogdet([[1.0, 2.0], [2.0, 4.0]]))
print(np.linalg.det(np.zeros((3, 3))), np.linalg.det([[5]]), np.linalg.matrix_power(np.array([[2.0, 1.0], [1.0, 1.0]]), -2))
for bad in ([[1.0, 2.0], [2.0, 4.0]], np.zeros((2, 2))):
    try:
        np.linalg.inv(bad)
    except np.linalg.LinAlgError as e:
        print("LinAlgError:", e)
try:
    np.linalg.solve(np.eye(2), np.ones(3))
except ValueError as e:
    print("ValueError:", e)
try:
    np.linalg.det(np.ones((2, 3)))
except np.linalg.LinAlgError as e:
    print("LinAlgError:", e)
g = np.random.default_rng(4)
for n in (2, 3, 5, 8, 13, 24, 40):
    M = g.uniform(-5, 5, (n, n))
    v = g.uniform(-5, 5, n)
    x = np.linalg.solve(M, v)
    print(n, f"{np.linalg.det(M):.9g}", np.round(x[:4], 8), np.round(np.linalg.inv(M)[0, :3], 8), np.allclose(M @ x, v))
