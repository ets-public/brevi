# Brevi's safetensors.numpy: save, save_file, load and load_file of the safetensors 0.8 package, for arrays of
# Brevi's numpy (float64, float32, int64, int32, bool, and the storage dtypes float16, int8, uint8, int16, uint16,
# uint32, uint64, complex64 that loading gives). The file's bytes are those the package writes: tensors
# sorted by dtype then name, the header padded to 8 bytes (metadata keys sorted; the package's order is its
# HashMap's). load returns the tensors in the file's order (the package's is its HashMap's too).
import numpy as np
import _brevi_fs as _fs
import _brevi_st as _st
from safetensors import SafetensorError, safe_open


def _flatten(tensor_dict, metadata):
    items = []
    for k, v in tensor_dict.items():
        if isinstance(v, np.ndarray):
            items.append((k, v))
        elif isinstance(v, np.generic):
            raise AttributeError("'numpy." + v.dtype.name + "' object has no attribute 'ctypes'")
        else:
            raise AttributeError("'" + type(v).__name__ + "' object has no attribute 'dtype'")
    for k, v in items:
        if not isinstance(k, str):
            raise TypeError("argument 'tensor_dict': '" + type(k).__name__ + "' object is not an instance of 'str'")
    meta = None
    if metadata is not None:
        if not isinstance(metadata, dict):
            raise TypeError("argument 'metadata': '" + type(metadata).__name__ + "' object is not an instance of 'dict'")
        meta = []
        for k, v in metadata.items():
            for x in (k, v):
                if not isinstance(x, str):
                    raise TypeError("argument 'metadata': '" + type(x).__name__ + "' object is not an instance of 'str'")
            meta.append((k, v))
    return _st.serialize(items, meta)


def save(tensor_dict, metadata=None):
    return _flatten(tensor_dict, metadata)


def save_file(tensor_dict, filename, metadata=None):
    data = _flatten(tensor_dict, metadata)
    path = filename.__fspath__() if hasattr(filename, "__fspath__") else filename
    try:
        _fs.write_bytes(path, data)
    except PermissionError:
        raise
    except OSError as e:
        raise SafetensorError("Error while serializing: I/O error: " + str(e))


def load(data):
    if not isinstance(data, type(b"")):
        raise TypeError("argument 'bytes': '" + type(data).__name__ + "' object is not an instance of 'bytes'")
    try:
        hdr = _st.header(data, len(data))
    except ValueError as e:
        raise SafetensorError("Error while deserializing: " + str(e))
    base = hdr[0] + 8
    out = {}
    for t in hdr[2]:
        out[t[0]] = _st.tensor(data[base + t[3]:base + t[4]], t[1], t[2], False)
    return out


def load_file(filename, backend="mmap"):
    with safe_open(filename, framework="np", backend=backend) as f:
        return f.get_tensors()
