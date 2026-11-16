# A Brevi build of this program copied to a CPU whose OpenBLAS kernels Brevi does not model must refuse every
# float BLAS/LAPACK call with NotImplementedError (rt_np.rs np_blas_ok). check.sh runs it under qemu-x86_64 -cpu SandyBridge.
import numpy as np
a = np.array([[2.0, 1.0, 0.5], [1.0, 3.0, 0.25], [0.5, 0.25, 4.0]])
v = np.array([1.0, 2.0, 3.0])
tests = [
    ("a @ a", lambda: a @ a),
    ("a @ v", lambda: a @ v),
    ("v @ v", lambda: v @ v),
    ("a.dot(a)", lambda: a.dot(a)),
    ("np.dot", lambda: np.dot(v, v)),
    ("np.inner", lambda: np.inner(v, v)),
    ("np.vdot", lambda: np.vdot(v, v)),
    ("np.matmul", lambda: np.matmul(a, a)),
    ("a @ a.T", lambda: a @ a.T),
    ("np.cov", lambda: np.cov(a)),
    ("np.corrcoef", lambda: np.corrcoef(a)),
    ("np.polyfit", lambda: np.polyfit(v, v * 2.0, 1)),
    ("norm", lambda: np.linalg.norm(v)),
    ("norm fro", lambda: np.linalg.norm(a)),
    ("det", lambda: np.linalg.det(a)),
    ("slogdet", lambda: np.linalg.slogdet(a)),
    ("solve", lambda: np.linalg.solve(a, v)),
    ("inv", lambda: np.linalg.inv(a)),
    ("lstsq", lambda: np.linalg.lstsq(a, v, rcond=None)),
]
for name, fn in tests:
    try:
        r = fn()
        print("RAN", name)
    except NotImplementedError:
        print("refused", name)
x = a.copy()
try:
    x @= a
    print("RAN @=")
except NotImplementedError:
    print("refused @=")
print("int ok", np.array([[1, 2], [3, 4]]) @ np.array([[1, 0], [0, 1]]))
