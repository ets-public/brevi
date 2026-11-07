# Builds the small ONNX models the tests in tests/onnxruntime run (needs the onnx package):
#   python3 tests/onnxruntime/models/make_models.py
import os

import numpy as np
import onnx
from onnx import TensorProto as T
from onnx import helper as h
from onnx import numpy_helper as nh

here = os.path.dirname(os.path.abspath(__file__))
rng = np.random.default_rng(7)


def f32(*shape):
    return rng.standard_normal(shape).astype(np.float32)


def save(m, name, **kw):
    onnx.checker.check_model(m)
    onnx.save(m, os.path.join(here, name), **kw)


def model(g, opset=17, ml=False, **kw):
    ops = [h.make_opsetid("", opset)] + ([h.make_opsetid("ai.onnx.ml", 3)] if ml else [])
    m = h.make_model(g, opset_imports=ops, **kw)
    m.ir_version = 9
    return m


# a two-layer classifier: probabilities and labels
nodes = [
    h.make_node("MatMul", ["x", "W1"], ["h1"]), h.make_node("Add", ["h1", "B1"], ["h2"]), h.make_node("Relu", ["h2"], ["h3"]),
    h.make_node("MatMul", ["h3", "W2"], ["h4"]), h.make_node("Add", ["h4", "B2"], ["logits"]),
    h.make_node("Softmax", ["logits"], ["probs"], axis=1), h.make_node("ArgMax", ["probs"], ["label"], axis=1, keepdims=0),
]
g = h.make_graph(nodes, "mlp", [h.make_tensor_value_info("x", T.FLOAT, ["batch", 4])],
                 [h.make_tensor_value_info("probs", T.FLOAT, ["batch", 3]), h.make_tensor_value_info("label", T.INT64, ["batch"])],
                 [nh.from_array(f32(4, 16), "W1"), nh.from_array(f32(16), "B1"), nh.from_array(f32(16, 3), "W2"), nh.from_array(f32(3), "B2")],
                 doc_string="two dense layers")
m = model(g, producer_name="brevi-tests", producer_version="1.0", domain="org.brevi", model_version=3, doc_string="a tiny classifier")
h.set_model_props(m, {"author": "brevi", "task": "classify"})
save(m, "mlp.onnx")

# a small convolutional net on 1x8x8 images
nodes = [
    h.make_node("Conv", ["img", "K", "KB"], ["c1"], pads=[1, 1, 1, 1], kernel_shape=[3, 3]), h.make_node("Relu", ["c1"], ["c2"]),
    h.make_node("MaxPool", ["c2"], ["p1"], kernel_shape=[2, 2], strides=[2, 2]),
    h.make_node("BatchNormalization", ["p1", "g", "b", "mu", "var"], ["bn"], epsilon=1e-5),
    h.make_node("Flatten", ["bn"], ["f"], axis=1), h.make_node("Gemm", ["f", "FW", "FB"], ["o"], transB=1),
    h.make_node("Sigmoid", ["o"], ["out"]),
]
g = h.make_graph(nodes, "cnn", [h.make_tensor_value_info("img", T.FLOAT, ["N", 1, 8, 8])], [h.make_tensor_value_info("out", T.FLOAT, ["N", 5])],
                 [nh.from_array(f32(4, 1, 3, 3), "K"), nh.from_array(f32(4), "KB"), nh.from_array(np.abs(f32(4)) + 0.5, "g"), nh.from_array(f32(4), "b"),
                  nh.from_array(f32(4), "mu"), nh.from_array(np.abs(f32(4)) + 0.5, "var"), nh.from_array(f32(5, 64), "FW"), nh.from_array(f32(5), "FB")])
save(model(g), "cnn.onnx")

# token ids -> embeddings -> attention-style mixing -> layer norm, GELU, pooled
D = 8
nodes = [
    h.make_node("Gather", ["E", "ids"], ["e"]),
    h.make_node("MatMul", ["e", "Wq"], ["q"]), h.make_node("MatMul", ["e", "Wk"], ["k"]),
    h.make_node("Transpose", ["k"], ["kt"], perm=[0, 2, 1]), h.make_node("MatMul", ["q", "kt"], ["s0"]),
    h.make_node("Div", ["s0", "scale"], ["s"]), h.make_node("Softmax", ["s"], ["a"], axis=-1), h.make_node("MatMul", ["a", "e"], ["ctx"]),
    h.make_node("LayerNormalization", ["ctx", "lg", "lb"], ["ln"], axis=-1, epsilon=1e-5),
    h.make_node("Gelu", ["ln"], ["gl"]), h.make_node("ReduceMean", ["gl", "ax"], ["pooled"], keepdims=0),
    h.make_node("Shape", ["ids"], ["shape"]),
]
g = h.make_graph(nodes, "encoder", [h.make_tensor_value_info("ids", T.INT64, ["batch", "seq"])],
                 [h.make_tensor_value_info("pooled", T.FLOAT, ["batch", D]), h.make_tensor_value_info("a", T.FLOAT, ["batch", "seq", "seq"]),
                  h.make_tensor_value_info("shape", T.INT64, [2])],
                 [nh.from_array(f32(50, D), "E"), nh.from_array(f32(D, D), "Wq"), nh.from_array(f32(D, D), "Wk"), nh.from_array(np.array(np.sqrt(D), np.float32), "scale"),
                  nh.from_array(np.ones(D, np.float32), "lg"), nh.from_array(np.zeros(D, np.float32), "lb"), nh.from_array(np.array([1], np.int64), "ax")])
save(model(g, opset=20), "encoder.onnx")

# a classifier the way sklearn-onnx writes one: a label and ZipMap's list of dicts
for labels, name in [(None, "zipmap_int.onnx"), (["cat", "dog", "owl"], "zipmap_str.onnx")]:
    nodes = [h.make_node("MatMul", ["x", "W"], ["z"]), h.make_node("Softmax", ["z"], ["probabilities"], axis=1),
             h.make_node("ArgMax", ["probabilities"], ["idx"], axis=1, keepdims=0)]
    if labels is None:
        nodes.append(h.make_node("Identity", ["idx"], ["label"]))
        nodes.append(h.make_node("ZipMap", ["probabilities"], ["output_probability"], domain="ai.onnx.ml", classlabels_int64s=[0, 1, 2]))
        lt, mt = T.INT64, h.make_map_type_proto(T.INT64, h.make_tensor_type_proto(T.FLOAT, []))
    else:
        nodes.append(h.make_node("LabelEncoder", ["idx"], ["label"], domain="ai.onnx.ml", keys_int64s=[0, 1, 2], values_strings=labels))
        nodes.append(h.make_node("ZipMap", ["probabilities"], ["output_probability"], domain="ai.onnx.ml", classlabels_strings=labels))
        lt, mt = T.STRING, h.make_map_type_proto(T.STRING, h.make_tensor_type_proto(T.FLOAT, []))
    g = h.make_graph(nodes, "zip", [h.make_tensor_value_info("x", T.FLOAT, [None, 3])],
                     [h.make_tensor_value_info("label", lt, [None]), h.make_value_info("output_probability", h.make_sequence_type_proto(mt))],
                     [nh.from_array(f32(3, 3), "W")])
    save(model(g, ml=True), name)

# several input and output types: float64, int32, bool
nodes = [
    h.make_node("Mul", ["a", "a"], ["sq"]), h.make_node("Cast", ["n"], ["nd"], to=T.DOUBLE), h.make_node("Add", ["sq", "nd"], ["sum"]),
    h.make_node("Greater", ["sum", "thr"], ["big"]), h.make_node("And", ["big", "mask"], ["both"]),
    h.make_node("Where", ["both", "sum", "zero"], ["kept"]), h.make_node("Cast", ["kept"], ["ki"], to=T.INT32),
    h.make_node("Mul", ["n", "two"], ["n2"]), h.make_node("CumSum", ["kept", "axis"], ["cs"]),
]
g = h.make_graph(nodes, "types", [h.make_tensor_value_info("a", T.DOUBLE, [3]), h.make_tensor_value_info("n", T.INT32, [3]), h.make_tensor_value_info("mask", T.BOOL, [3])],
                 [h.make_tensor_value_info("kept", T.DOUBLE, [3]), h.make_tensor_value_info("both", T.BOOL, [3]), h.make_tensor_value_info("ki", T.INT32, [3]),
                  h.make_tensor_value_info("n2", T.INT32, [3]), h.make_tensor_value_info("cs", T.DOUBLE, [3])],
                 [nh.from_array(np.array(2.0), "thr"), nh.from_array(np.array(0.0), "zero"), nh.from_array(np.array(2, np.int32), "two"), nh.from_array(np.array(0, np.int32), "axis")])
save(model(g), "types.onnx")

# weights kept in a file next to the model (external data)
W = f32(6, 6)
g = h.make_graph([h.make_node("MatMul", ["x", "W"], ["y"]), h.make_node("Tanh", ["y"], ["t"])], "ext", [h.make_tensor_value_info("x", T.FLOAT, [1, 6])],
                 [h.make_tensor_value_info("t", T.FLOAT, [1, 6])], [nh.from_array(W, "W")])
m = model(g)
onnx.save(m, os.path.join(here, "external.onnx"), save_as_external_data=True, all_tensors_to_one_file=True, location="external.weights", size_threshold=0)

with open(os.path.join(here, "broken.onnx"), "wb") as f:
    f.write(b"this is not an onnx model")
