# onnxruntime errors: bad inputs, unknown outputs, models that fail to load
import numpy as np
import onnxruntime as ort

M = "tests/onnxruntime/models/"
s = ort.InferenceSession(M + "mlp.onnx")
x = np.arange(8, dtype=np.float32).reshape(2, 4)
for bad in [{"x": x.astype(np.float64)}, {"y": x}, {"x": x.reshape(4, 2)}, {}, {"x": x.astype(np.int32)}, {"x": x[0]}]:
    try:
        s.run(None, bad)
    except Exception as e:
        print(type(e).__name__, e)
for names in [["nope"], ["probs", "nope"], []]:
    try:
        print(len(s.run(names, {"x": x})))
    except Exception as e:
        print(type(e).__name__, e)
for p in [M + "broken.onnx", M + "nope.onnx", b"garbage", b""]:
    try:
        ort.InferenceSession(p)
    except Exception as e:
        print(type(e).__name__, e)
try:
    ort.InferenceSession(5)
except Exception as e:
    print(type(e).__name__)
try:
    s.run(None, {"x": x}, 5)
except Exception as e:
    print(type(e).__name__)
