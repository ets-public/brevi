# Brevi's own numpy runs under --sandbox with no grant: it has no file, network or process access.
import numpy as np

a = np.arange(6, dtype=np.float64).reshape(2, 3)
print(a.sum(axis=0), a.mean())
print(a @ a.T)
rng = np.random.default_rng(7)
print(rng.integers(0, 10, size=4))
print(np.linalg.det(np.array([[2.0, 1.0], [1.0, 3.0]])))
