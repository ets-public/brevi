# safe_open(...).get_slice: the indexing safetensors takes (ints, slices with steps, one ellipsis) and its errors
import numpy as np
from safetensors import safe_open
from safetensors.numpy import save_file

t = np.arange(2 * 3 * 4, dtype=np.float32).reshape(2, 3, 4)
save_file({"t": t, "v": np.arange(5), "e": np.zeros((0, 2)), "b": np.array([True, False, True, True])}, "/tmp/brevi_st_slice.safetensors")


def show(f):
    try:
        r = f()
        print(r.shape, r.dtype, r.tolist())
    except Exception as e:
        print(type(e).__name__, e)


with safe_open("/tmp/brevi_st_slice.safetensors", framework="np") as f:
    s = f.get_slice("t")
    v = f.get_slice("v")
    for idx in [0, 1, -1, -2, (0, 1), (1, 2, 3), (slice(None), 1), (Ellipsis, 2), (0, Ellipsis), (Ellipsis,), (slice(0, 2), slice(1, 3)),
                (slice(None), slice(None), slice(None, None, 2)), (slice(None, None, 2),), (slice(1, None), slice(None, 2), slice(1, 4, 2)),
                (0, Ellipsis, slice(1, 2)), (), np.int64(1), True]:
        show(lambda: s[idx])
    for idx in [slice(1, 4), slice(None, None, 3), 4, -5, slice(0, 0), slice(2, 2), slice(0, 6), 5, -6, (0, 0), (Ellipsis, Ellipsis),
                slice(None, None, 0), slice(None, None, -1), slice(-1, None), "x", [0], [], 2 ** 40, 1.5]:
        show(lambda: v[idx])
    show(lambda: f.get_slice("e")[:])
    show(lambda: f.get_slice("e")[0:0])
    show(lambda: f.get_slice("b")[1:3])
    show(lambda: f.get_slice("t")[3])
    show(lambda: f.get_slice("t")[:, 3])
    show(lambda: f.get_slice("t")[:, :, 0:5])
    print(s.get_shape(), s.get_dtype(), v.get_dtype())
