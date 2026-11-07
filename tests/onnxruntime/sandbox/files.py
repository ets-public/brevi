import onnxruntime as ort

# A model file needs the read grant, and writing an optimized model or a profile the write grant: under --sandbox
# the build refuses this program without them.
s = ort.InferenceSession("tests/onnxruntime/models/mlp.onnx")
so = ort.SessionOptions()
so.optimized_model_filepath = "/tmp/brevi_sb_opt.onnx"
so.enable_profiling = True
