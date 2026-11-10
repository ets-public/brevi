import numpy as np
e = np.array([0.0, -0.0, 1e-30, -1e-30, 88.7, 88.72283, 88.73, -103.9, -103.97, -104.0, -87.5, 1e10, -1e10, 3.14159, 71476.0, 71477.0, 117435.0, 117437.0, 1e-45, -1e-45, 0.5, -0.5, -0.999, 2.0, 1e20, 3e38, np.inf, -np.inf, np.nan], dtype=np.float32)
print(np.exp(e).tolist())
print(np.tanh(e).tolist())
print(np.sin(e).tolist())
print(np.cos(e).tolist())
print(np.log1p(np.abs(e)).tolist())
print(np.log1p(np.maximum(e, np.float32(-1.0))).tolist())
s = e[::3]
print(np.exp(s), np.sin(s), np.log1p(np.abs(s)))
m = np.abs(e[:12]).reshape(3, 4)
print(np.tanh(m), np.cos(m).dtype, np.linalg.norm(m), np.linalg.norm(m[0]))
print(np.exp(np.float32(1.5)), np.sin(np.float32(2.0)))
# many values across the ranges (numpy's float32 SIMD loops, bit for bit)
base = np.arange(20000, dtype=np.float64)
for scale in (12.0, 0.9999, 1e-6, 1e5, 110.0):
    x = (np.sin(base * 1.37) * scale).astype(np.float32)
    print(scale, np.exp(x).astype(np.float64).sum(), np.tanh(x).astype(np.float64).sum(), np.sin(x).astype(np.float64).sum(), np.cos(x).astype(np.float64).sum(), np.log1p(np.maximum(x, np.float32(-0.9999))).astype(np.float64).sum())
