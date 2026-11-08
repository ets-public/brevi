# safetensors tensors of the dtypes Brevi's numpy holds without computing on them (float16, 8/16-bit and unsigned
# integers, complex64): shape, dtype, printing, indexing, reshaping, astype to a computing type, saving back.
# Files are written by hand (the program never names those dtypes, as a program loading model weights doesn't).
import json

import numpy as np
from safetensors import safe_open
from safetensors.numpy import load, save, save_file


def build(entries):
    header = {}
    blob = b""
    for name, dtype, shape, raw in entries:
        header[name] = {"dtype": dtype, "shape": shape, "data_offsets": [len(blob), len(blob) + len(raw)]}
        blob += raw
    h = json.dumps(header, separators=(",", ":")).encode()
    h += b" " * ((8 - len(h) % 8) % 8)
    n = len(h)
    return bytes([n % 256, n // 256, 0, 0, 0, 0, 0, 0]) + h + blob


# float16 0.1, -2.5, 65504, 6.1e-05, 1/3, 0, -0, 1e-07 (little-endian), and the other types' values as bytes
halves = b"f.\x00\xc1\xff{\xff\x03U5\x00\x00\x00\x80\x02\x00"
data = build([
    ("h", "F16", [2, 4], halves),
    ("i8", "I8", [5], b"\x80\xff\x00\x07\x7f"),
    ("u8", "U8", [2, 3], bytes([0, 1, 2, 128, 254, 255])),
    ("i16", "I16", [3], b"\x00\x80\x0c\x00\xff\x7f"),
    ("u16", "U16", [3], b"\x00\x00@\x9c\xff\xff"),
    ("u32", "U32", [2], b"\x07\x00\x00\x00\x00(k\xee"),
    ("u64", "U64", [2], b"\x03" + b"\x00" * 7 + b"\xff" * 8),
    ("c", "C64", [2], b"\x00\x00\xc0?\x00\x00\x00\xc0\x00\x00\x00\x00\x00\x00P@"),
])
t = load(data)
for k in sorted(t):
    a = t[k]
    print(k, a.dtype, a.shape, a.ndim, a.size, a.itemsize, a.nbytes, a.dtype.name, a.dtype.itemsize)
h = t["h"]
print(h)
print(repr(h))
print(repr(h[0]), h[0, 1], repr(h[1, 2:]), h.T.shape, repr(h.reshape(4, 2)[1]))
print(h.tolist())
print(h.astype(np.float32))
print(h.astype(np.float64).sum(), h.astype(np.float32).dtype, float(h[0, 0]), int(h[0, 2]))
for k in ["i8", "u8", "i16", "u16", "u32", "u64"]:
    a = t[k]
    print(repr(a), a, a.tolist(), repr(a[-1]), a.astype(np.int64), a.astype(np.float64))
print(t["u8"].flatten().tolist(), t["u8"].ravel().shape, t["u8"].copy().dtype, len(t["u8"]))
print([repr(x) for x in t["i8"]], list(t["u16"]))
print(t["c"].dtype, t["c"].real, t["c"].imag)
back = load(save(t))
print(sorted((k, back[k].dtype.name, back[k].shape) for k in back))
print(save(t) == data, save({"h": h[:, 1:3]})[8:80])
print(load(save({"part": t["u8"][1]}))["part"])

save_file(t, "/tmp/brevi_st_dtypes.safetensors")
with safe_open("/tmp/brevi_st_dtypes.safetensors", framework="np") as f:
    print(f.get_slice("h").get_dtype(), repr(f.get_slice("h")[1:, ::2]), repr(f.get_tensor("u16")))
    print(f.get_slice("i8")[1:4], f.get_slice("u64")[1:])

for name, raw in [("BF16", b"\x80\x3f\x00\x40"), ("F8_E4M3", b"\x38\x40")]:
    n = len(raw) // (2 if name == "BF16" else 1)
    try:
        print(load(build([("w", name, [n], raw)])))
    except Exception as e:
        print(name, type(e).__name__, e)
