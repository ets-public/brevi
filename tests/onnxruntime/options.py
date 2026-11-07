# onnxruntime SessionOptions, RunOptions and enums, a session from model bytes, provider lists
import numpy as np
import onnxruntime as ort

M = "tests/onnxruntime/models/"
so = ort.SessionOptions()
print(so.graph_optimization_level, repr(so.execution_mode), so.intra_op_num_threads, so.inter_op_num_threads)
print(so.enable_cpu_mem_arena, so.enable_mem_pattern, so.log_severity_level, so.enable_profiling, so.use_deterministic_compute)
so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
so.intra_op_num_threads = 1
so.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
so.log_severity_level = 3
so.add_session_config_entry("session.use_env_allocators", "0")
print(so.graph_optimization_level, int(ort.GraphOptimizationLevel.ORT_ENABLE_EXTENDED), so.get_session_config_entry("session.use_env_allocators"))
x = np.arange(8, dtype=np.float32).reshape(2, 4) / 7
s = ort.InferenceSession(M + "mlp.onnx", so, providers=["CPUExecutionProvider"])
print(s.run(None, {"x": x})[0])
ro = ort.RunOptions()
ro.log_severity_level = 3
ro.logid = "brevi"
print(ro.logid, ro.log_severity_level, ro.terminate)
print(s.run(["label"], {"x": x}, ro))
s2 = ort.InferenceSession(M + "mlp.onnx", sess_options=so, providers=[("CPUExecutionProvider", {})])
print(s2.get_providers(), s2.get_provider_options())
print(ort.GraphOptimizationLevel.ORT_ENABLE_ALL == ort.GraphOptimizationLevel.ORT_ENABLE_ALL, str(ort.ExecutionMode.ORT_PARALLEL))
