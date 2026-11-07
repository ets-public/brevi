# safetensors' errors: headers it refuses (serde_json's messages and positions), bad arguments, missing files
import numpy as np
from safetensors import SafetensorError, safe_open
from safetensors.numpy import load, load_file, save, save_file


def show(f):
    try:
        r = f()
        print("ok", sorted(r) if isinstance(r, dict) else r)
    except Exception as e:
        print(type(e).__name__, e)


def raw(h, data=b""):
    hb = h.encode()
    n = len(hb)
    return bytes([n % 256, n // 256, 0, 0, 0, 0, 0, 0]) + hb + data


A = '{"dtype":"F32","shape":[1],"data_offsets":[0,4]}'
headers = [
    ('{"a":{"dtype":"XX","shape":[1],"data_offsets":[0,4]}}', b"abcd"),
    ('{"a":{"dtype":"F32","shape":[1]}}', b"abcd"),
    ('{"a":{"dtype":"F32","shape":1,"data_offsets":[0,4]}}', b"abcd"),
    ('{"a":{"dtype":"F32","shape":[-1],"data_offsets":[0,4]}}', b"abcd"),
    ('{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4,5]}}', b"abcd"),
    ('{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4],"x":1}}', b"abcd"),
    ('{"a":{"dtype":"F32","shape":[2],"data_offsets":[0,4]}}', b"abcd"),
    ('{"a":{"dtype":"F32","shape":[1],"data_offsets":[4,8]}}', b"abcdabcd"),
    ('{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}', b"abcdabcd"),
    ('{"a":5}', b""),
    ('{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}', b"abcd"),
    ('{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}} x', b"abcd"),
    ('{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}   ', b"abcd"),
    ('{"__metadata__":{"x":"y"},"__metadata__":{}}', b""),
    ('\n{"a":\n {"dtype":"F32"}}', b""),
    ('{"a":{"dtype":5,"shape":[1],"data_offsets":[0,4]}}', b""),
    ('{"a":{"dtype":"F32","shape":"x","data_offsets":[0,4]}}', b""),
    ('{"a":{"dtype":"F32","shape":[1],"data_offsets":5}}', b""),
    ('{"a":%s,"b":{"dtype":"F32","shape":[-1],"data_offsets":[4,8]},"c":%s}' % (A, A), b""),
    ('{"a":%s,"b":{"dtype":"F32","shape":[1, 2.5],"data_offsets":[4,8]},"c":{"shape":[1]}}' % A, b""),
    ('{"a":{"shape":[1],"dtype":"F32","data_offsets":[0]  }  }', b""),
    ('{"a":{"shape":[1],"dtype":"F32","data_offsets":[0,4],"dtype":"F32"  }  }', b""),
    ('{"a":{"shape":[1],"dtype":"F32","data_offsets":[0,true]  }  }', b""),
    ('{"a":{"shape":[1],"dtype":"F32","data_offsets":[0,"4"]  }  }', b""),
    ('{"a":[1,2]  }', b""),
    ('{"a":null  }', b""),
    ('{"__metadata__":[]}', b""),
    ('{"__metadata__":{"a":null}}', b""),
    ('{"__metadata__":5}', b""),
    ('{"__metadata__":null}', b""),
    ('[]', b""), ('5', b""), ('"x"', b""), ('null', b""), ('{"a":1e400}', b""), ('{"a":tru}', b""),
    ('{"a":"\\x"}', b""), ('{"a":"\x01"}', b""), ('{1:2}', b""), ('{"a" 2}', b""), ('{"a":2 "b"}', b""), ('{"a":[1 2]}', b""),
    ('', b""), ('   ', b""), ('{"a":-}', b""), ('{"a":01}', b""), ('{"a":1.}', b""), ('{"a":[1,]}', b""), ('{"a":1,}', b""),
    ('{"\\u00e9":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}', b"\x00\x00\x80?"),
    ('{"a":{"dtype":"F4","shape":[3],"data_offsets":[0,2]}}', b"\x00<"),
]
for h, d in headers:
    show(lambda: load(raw(h, d)))
show(lambda: load(b"abc"))
show(lambda: load(b"\x05\x00\x00\x00\x00\x00\x00\x00abc"))
show(lambda: load(b"\xff\xff\xff\xff\x00\x00\x00\x00abc"))
show(lambda: load(b"\x02\x00\x00\x00\x00\x00\x00\x00\xff\xfe"))
show(lambda: load("text"))

show(lambda: load_file("/tmp/brevi_st_missing.safetensors"))
show(lambda: safe_open("/tmp/brevi_st_missing.safetensors", framework="np"))
save_file({"a": np.ones(2)}, "/tmp/brevi_st_err.safetensors")
show(lambda: safe_open("/tmp/brevi_st_err.safetensors", framework="nope"))
show(lambda: safe_open("/tmp/brevi_st_err.safetensors", framework="np").get_tensor("zz"))
show(lambda: safe_open("/tmp/brevi_st_err.safetensors", framework="np").get_slice("zz"))
show(lambda: safe_open("/tmp/brevi_st_err.safetensors", framework="np", backend="disk"))
with safe_open("/tmp/brevi_st_err.safetensors", framework="np") as f:
    pass
show(lambda: f.keys())
show(lambda: f.get_tensor("a"))
show(lambda: save({"a": [1, 2]}))
show(lambda: save({"a": np.float64(1)}))
show(lambda: save({1: np.ones(2)}))
show(lambda: save({"a": np.ones(2)}, metadata={"x": 1}))
show(lambda: save({"a": np.ones(2)}, metadata={2: "x"}))
show(lambda: len(save({"__metadata__": np.ones(2)})))
show(lambda: save({}))
show(lambda: load(save({})))
