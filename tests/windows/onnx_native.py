import onnxruntime as ort

s = ort.InferenceSession("tests/onnxruntime/models/mlp.onnx")
for i in s.get_inputs():
    print(i.name, i.type, i.shape)
for o in s.get_outputs():
    print(o.name, o.type, o.shape)
probs, label = s.run(None, {"x": [[1.0, 2.0, 3.0, 4.0]]})
print(probs)
print(label)
print(probs.dtype, label.dtype)
