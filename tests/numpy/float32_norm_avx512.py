# np.linalg.norm of float32 arrays is sqrt(x.ravel('K').dot(x)) in numpy: OpenBLAS's sdot, not a pairwise sum
import numpy as np

for n in [7, 33, 100, 1000, 4097]:
    x = (np.arange(n, dtype=np.float32) * np.float32(0.37)) % np.float32(1.9) - np.float32(0.8)
    m = x[: (n // 3) * 3].reshape(3, n // 3)
    print(n, float(np.linalg.norm(x)), float(np.linalg.norm(x[::2])), float(np.linalg.norm(m)), float(np.linalg.norm(m.T)), np.linalg.norm(x).dtype)
