# As blas_refusal.py for the float32 kernels (SkylakeX only): refused on a Haswell or older CPU.
import numpy as np
f = np.array([1.0, 2.0, 3.0], dtype=np.float32)
for name, fn in [("norm f32", lambda: np.linalg.norm(f)), ("f @ f", lambda: f @ f), ("f.dot", lambda: f.dot(f))]:
    try:
        fn()
        print("RAN", name)
    except NotImplementedError:
        print("refused", name)
