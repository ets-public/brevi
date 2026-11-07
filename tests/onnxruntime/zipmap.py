# onnxruntime ZipMap outputs: sequences of maps (int and str keys) come back as lists of dicts of floats
import numpy as np
import onnxruntime as ort

M = "tests/onnxruntime/models/"
s = ort.InferenceSession(M + "zipmap_int.onnx")
r = s.run(None, {"x": np.array([[1, 2, 3], [0.5, -1, 2]], np.float32)})
print(r)
print([(o.name, o.type, o.shape) for o in s.get_outputs()])
for d in r[1]:
    print(sorted(d.items()), max(d, key=d.get), type(d[0]).__name__)
s2 = ort.InferenceSession(M + "zipmap_str.onnx")
print(s2.run(["output_probability"], {"x": np.array([[1, 2, 3]], np.float32)}))
print([(o.name, o.type) for o in s2.get_outputs()])
