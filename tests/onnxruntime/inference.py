# onnxruntime inference: an MLP, a CNN, a small encoder and a graph of mixed types, run on numpy inputs
import numpy as np
import onnxruntime as ort

M = "tests/onnxruntime/models/"
s = ort.InferenceSession(M + "mlp.onnx")
for a in s.get_inputs() + s.get_outputs():
    print(a.name, a.type, a.shape, str(a))
m = s.get_modelmeta()
print(m.producer_name, m.graph_name, m.domain, m.description, m.graph_description, m.version, m.custom_metadata_map)
print(s.get_providers(), ort.get_available_providers(), ort.get_device())
x = np.arange(8, dtype=np.float32).reshape(2, 4) / 7
probs, label = s.run(None, {"x": x})
print(probs, label)
print(probs.dtype, label.dtype, probs.sum(axis=1))
print(s.run(["label"], {"x": [[1, 2, 3, 4]]}))
print(s.run(["probs"], {"x": x[:1]})[0].argmax())

e = ort.InferenceSession(M + "encoder.onnx")
p, a, sh = e.run(None, {"ids": np.array([[1, 2, 3], [4, 5, 6]])})
print(p.round(5), a.round(4), sh)  # float32 last digits move between onnxruntime releases

t = ort.InferenceSession(M + "types.onnx")
print([(i.name, i.type) for i in t.get_inputs()], [(o.name, o.type) for o in t.get_outputs()])
print(t.run(None, {"a": np.array([1.5, -2.0, 0.25]), "n": np.array([1, 7, 2], np.int32), "mask": np.array([True, True, False])}))

c = ort.InferenceSession(M + "cnn.onnx")
img = np.linspace(-1, 1, 128, dtype=np.float32).reshape(2, 1, 8, 8)
out = c.run(None, {"img": img})[0]
print(out.shape, out)

w = ort.InferenceSession(M + "external.onnx")
print(w.run(None, {"x": np.ones((1, 6), np.float32)}))

