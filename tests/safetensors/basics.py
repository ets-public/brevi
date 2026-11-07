# safetensors with numpy: save / load round trips, the file's bytes, safe_open
import hashlib

import numpy as np
from safetensors import safe_open
from safetensors.numpy import load, load_file, save, save_file

tensors = {
    "weight": np.arange(12, dtype=np.float32).reshape(3, 4) / 3,
    "bias": np.array([0.5, -1.25, 2.0]),
    "ids": np.array([[1, 2], [3, 4]]),
    "small": np.array([7, -8, 9], dtype=np.int32),
    "mask": np.array([True, False, True]),
    "scalar": np.array(3.5, dtype=np.float32),
    "empty": np.zeros((0, 3), dtype=np.float32),
    "a.b/c": np.ones(2),
}
data = save(tensors)
print(len(data), data[:8], hashlib.sha256(data).hexdigest())
header = data[8:8 + data[0] + 256 * data[1]]
print(header)
back = load(data)
for k in sorted(back):
    v = back[k]
    print(k, v.dtype, v.shape, v.tolist())

with_meta = save({"x": np.linspace(0, 1, 4)}, metadata={"format": "np"})
print(with_meta[8:8 + with_meta[0] + 256 * with_meta[1]])

save_file(tensors, "/tmp/brevi_st_basics.safetensors", metadata={"producer": "brevi tests"})
loaded = load_file("/tmp/brevi_st_basics.safetensors")
print(list(loaded))
print(loaded["weight"])
print(repr(loaded["bias"]), repr(loaded["small"]), repr(loaded["scalar"]), repr(loaded["empty"]))
print(loaded["weight"].sum(), loaded["ids"] * 2, loaded["mask"].any())

with safe_open("/tmp/brevi_st_basics.safetensors", framework="np") as f:
    print(f.keys())
    print(f.offset_keys())
    print(f.metadata())
    print(f.get_tensor("ids"))
    t = f.get_tensors()
    print(list(t), t["mask"])

f = safe_open("/tmp/brevi_st_basics.safetensors", "numpy", device="cpu")
print(f.get_tensor("small").dtype, f.get_slice("weight").get_shape(), f.get_slice("weight").get_dtype())

# a model's weights, the usual way: a dict of float32 arrays, read back by name
rng = np.random.default_rng(3)
layers = {}
for i in range(3):
    layers[f"layers.{i}.weight"] = rng.standard_normal((4, 4)).astype(np.float32)
    layers[f"layers.{i}.bias"] = np.zeros(4, dtype=np.float32)
save_file(layers, "/tmp/brevi_st_model.safetensors", metadata={"format": "np"})
x = np.ones(4, dtype=np.float32)
with safe_open("/tmp/brevi_st_model.safetensors", framework="np") as f:
    for i in range(3):
        x = np.maximum(f.get_tensor(f"layers.{i}.weight") * x + f.get_tensor(f"layers.{i}.bias"), 0).sum(axis=1)
print(x, x.dtype)
print(hashlib.sha256(save(layers)).hexdigest())

# arrays whose memory is not in their order: safetensors writes the memory from the first element on
m = np.arange(6.0).reshape(2, 3)
print(load(save({"t": m.T}))["t"])
print(load(save({"s": m[:, ::2]}))["s"])
print(load(save({"r": m[1]}))["r"])
