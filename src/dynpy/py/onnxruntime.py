# Brevi's onnxruntime: the onnxruntime 1.29 package's Python API (InferenceSession, SessionOptions, RunOptions,
# NodeArg, ModelMetadata, the enums and module functions) over ONNX Runtime's own engine, through its C API
# (_brevi_ort), on the CPU. Inputs are arrays of Brevi's numpy (float32, float64, int64, int32, bool, str);
# results come back as onnxruntime's binding gives them: arrays, lists (sequences) and dicts (maps, ZipMap's).
# Under --sandbox a model file needs the read grant, and files the engine writes need the write grant.
# Not here (caps.rs ONNXRUNTIME_LACKS sends a program using them to CPython's onnxruntime): IO binding, OrtValue,
# sparse tensors, custom op libraries, LoRA adapters, model compilation, run_async, profiling results, other
# execution providers. Results of types Brevi's numpy lacks (float16, uint8, string tensors, ...) raise
# NotImplementedError.
import sys
import numpy as np
import _brevi_ort as _o
from onnxruntime.capi.onnxruntime_pybind11_state import (EngineError, EPFail, Fail, InvalidArgument, InvalidGraph, InvalidProtobuf,
                                                          ModelLoadCanceled, ModelLoaded, ModelRequiresCompilation, NoModel, NoSuchFile,
                                                          NotFound, NotImplementedStatus, RuntimeException)

__version__ = _o.load()
__author__ = "Microsoft"

_CODES = [None, (Fail, "FAIL"), (InvalidArgument, "INVALID_ARGUMENT"), (NoSuchFile, "NO_SUCHFILE"), (NoModel, "NO_MODEL"),
          (EngineError, "ENGINE_ERROR"), (RuntimeException, "RUNTIME_EXCEPTION"), (InvalidProtobuf, "INVALID_PROTOBUF"),
          (ModelLoaded, "MODEL_LOADED"), (NotImplementedStatus, "NOT_IMPLEMENTED"), (InvalidGraph, "INVALID_GRAPH"),
          (EPFail, "EP_FAIL"), (ModelLoadCanceled, "MODEL_LOAD_CANCELED"), (ModelRequiresCompilation, "MODEL_REQUIRES_COMPILATION"),
          (NotFound, "NOT_FOUND")]


def _raise(e):
    if not e.args or not isinstance(e.args[0], tuple):
        raise e
    code, msg = e.args[0]
    if code == -1:
        raise PermissionError(msg)
    if code == -2:
        raise ImportError(msg)
    if msg.startswith("Invalid output name: "):
        # the binding's own check words it so
        msg = "Invalid output name:" + msg[21:]
    if 0 < code < len(_CODES):
        cls, name = _CODES[code]
        raise cls("[ONNXRuntimeError] : " + str(code) + " : " + name + " : " + msg)
    raise Fail("[ONNXRuntimeError] : " + str(code) + " : FAIL : " + msg)


class _Enum:
    def __init__(self, name, value):
        self._name = name
        self._value = value

    @property
    def name(self):
        return self._name

    @property
    def value(self):
        return self._value

    def __int__(self):
        return self._value

    def __index__(self):
        return self._value

    def __repr__(self):
        return "<" + type(self).__name__ + "." + self._name + ": " + str(self._value) + ">"

    def __str__(self):
        return type(self).__name__ + "." + self._name

    def __eq__(self, other):
        return type(other) is type(self) and other._value == self._value

    def __ne__(self, other):
        return not self.__eq__(other)

    def __hash__(self):
        return hash(self._value)


class GraphOptimizationLevel(_Enum):
    pass


class ExecutionMode(_Enum):
    pass


class ExecutionOrder(_Enum):
    pass


def _members(cls, pairs):
    m = {}
    for n, v in pairs:
        x = cls(n, v)
        setattr(cls, n, x)
        m[n] = x
    cls.__members__ = m


_members(GraphOptimizationLevel, [("ORT_DISABLE_ALL", 0), ("ORT_ENABLE_BASIC", 1), ("ORT_ENABLE_EXTENDED", 2), ("ORT_ENABLE_LAYOUT", 3), ("ORT_ENABLE_ALL", 99)])
_members(ExecutionMode, [("ORT_SEQUENTIAL", 0), ("ORT_PARALLEL", 1)])
_members(ExecutionOrder, [("DEFAULT", 0), ("PRIORITY_BASED", 1), ("MEMORY_EFFICIENT", 2)])


class SessionOptions:
    def __init__(self):
        self.enable_cpu_mem_arena = True
        self.enable_mem_pattern = True
        self.enable_mem_reuse = True
        self.enable_profiling = False
        self.execution_mode = ExecutionMode.ORT_SEQUENTIAL
        self.execution_order = ExecutionOrder.DEFAULT
        self.graph_optimization_level = GraphOptimizationLevel.ORT_ENABLE_ALL
        self.inter_op_num_threads = 0
        self.intra_op_num_threads = 0
        self.log_severity_level = -1
        self.log_verbosity_level = 0
        self.logid = ""
        self.optimized_model_filepath = ""
        self.profile_file_prefix = "onnxruntime_profile_"
        self.use_deterministic_compute = False
        self._config = {}

    def add_session_config_entry(self, key, value):
        self._config[key] = value

    def get_session_config_entry(self, key):
        if key not in self._config:
            raise RuntimeError("SessionOptions does not have configuration with key: " + key)
        return self._config[key]

    def has_providers(self):
        return False

    def _pairs(self):
        return [("graph_optimization_level", int(self.graph_optimization_level)), ("intra_op_num_threads", int(self.intra_op_num_threads)),
                ("inter_op_num_threads", int(self.inter_op_num_threads)), ("execution_mode", int(self.execution_mode)),
                ("log_severity_level", int(self.log_severity_level)), ("log_verbosity_level", int(self.log_verbosity_level)),
                ("logid", self.logid), ("enable_mem_pattern", bool(self.enable_mem_pattern)), ("enable_cpu_mem_arena", bool(self.enable_cpu_mem_arena)),
                ("optimized_model_filepath", str(self.optimized_model_filepath)), ("enable_profiling", bool(self.enable_profiling)),
                ("profile_file_prefix", self.profile_file_prefix), ("use_deterministic_compute", bool(self.use_deterministic_compute)),
                ("config", list(self._config.items()))]


class RunOptions:
    def __init__(self):
        self.log_severity_level = -1
        self.log_verbosity_level = 0
        self.logid = ""
        self.terminate = False
        self.only_execute_path_to_fetches = False
        self._config = {}

    def add_run_config_entry(self, key, value):
        self._config[key] = value

    def get_run_config_entry(self, key):
        if key not in self._config:
            raise RuntimeError("RunOptions does not have configuration with key: " + key)
        return self._config[key]


class NodeArg:
    def __init__(self, name, type, shape):
        self.name = name
        self.type = type
        self.shape = shape

    def __str__(self):
        return "NodeArg(name=" + repr(self.name) + ", type=" + repr(self.type) + ", shape=" + repr(self.shape) + ")"

    def __repr__(self):
        return "<onnxruntime.capi.onnxruntime_pybind11_state.NodeArg object at " + hex(id(self)) + ">"


class ModelMetadata:
    def __init__(self, m):
        self.producer_name = m[0]
        self.graph_name = m[1]
        self.domain = m[2]
        self.description = m[3]
        self.graph_description = m[4]
        self.version = m[5]
        self.custom_metadata_map = m[6]


def get_available_providers():
    return _o.providers()


def get_all_providers():
    return ["TensorrtExecutionProvider", "CUDAExecutionProvider", "MIGraphXExecutionProvider", "NvTensorRTRTXExecutionProvider", "OpenVINOExecutionProvider",
            "DnnlExecutionProvider", "VitisAIExecutionProvider", "QNNExecutionProvider", "NnapiExecutionProvider", "VSINPUExecutionProvider",
            "JsExecutionProvider", "CoreMLExecutionProvider", "ArmNNExecutionProvider", "ACLExecutionProvider", "DmlExecutionProvider",
            "RknpuExecutionProvider", "WebNNExecutionProvider", "WebGpuExecutionProvider", "XnnpackExecutionProvider", "CANNExecutionProvider",
            "AzureExecutionProvider", "CPUExecutionProvider"]


def get_device():
    return "GPU" if "CUDAExecutionProvider" in _o.providers() else "CPU"


def get_version_string():
    return __version__


def get_build_info():
    return _o.build_info()


def set_default_logger_severity(level):
    pass


def _normalize_providers(providers, provider_options, available):
    if providers is None:
        return []
    names = []

    def add(name):
        if name not in available:
            print(".../onnxruntime_inference_collection.py:153: UserWarning: Specified provider '" + name + "' is not in available provider names.Available providers: '"
                  + ", ".join(available) + "'", file=sys.stderr)
        if name in names:
            print(".../onnxruntime_inference_collection.py:160: UserWarning: Duplicate provider '" + name + "' encountered, ignoring.", file=sys.stderr)
            return
        names.append(name)

    if not isinstance(providers, (list, tuple)):
        raise ValueError("'providers' should be a sequence.")
    if provider_options is not None:
        if not isinstance(provider_options, (list, tuple)):
            raise ValueError("'provider_options' should be a sequence.")
        if len(providers) != len(provider_options):
            raise ValueError("'providers' and 'provider_options' should be the same length if both are given.")
        for p in providers:
            if not isinstance(p, str):
                raise ValueError("Only string values for 'providers' are supported if 'provider_options' is given.")
        for o in provider_options:
            if not isinstance(o, dict):
                raise ValueError("'provider_options' values must be dicts.")
        for p in providers:
            add(p)
    else:
        for p in providers:
            if isinstance(p, str):
                add(p)
            elif isinstance(p, tuple) and len(p) == 2 and isinstance(p[0], str) and isinstance(p[1], dict):
                add(p[0])
            else:
                raise ValueError("'providers' values must be either strings or (string, dict) tuples.")
    return names


_NP = {"tensor(float)": np.float32, "tensor(double)": np.float64, "tensor(int64)": np.int64, "tensor(int32)": np.int32, "tensor(bool)": np.bool_,
       "tensor(string)": np.str_}


class InferenceSession:
    def __init__(self, path_or_bytes, sess_options=None, providers=None, provider_options=None, **kwargs):
        if isinstance(path_or_bytes, str) or isinstance(path_or_bytes, type(b"")):
            model = path_or_bytes
        elif hasattr(path_or_bytes, "__fspath__"):
            model = path_or_bytes.__fspath__()
        else:
            raise TypeError("Unable to load from type '" + str(type(path_or_bytes)) + "'")
        _normalize_providers(providers, provider_options, _o.providers())
        self._sess_options = sess_options if sess_options is not None else SessionOptions()
        try:
            r = _o.session(model, self._sess_options._pairs())
        except RuntimeError as e:
            _raise(e)
        self._id = r[0]
        self._inputs_meta = [NodeArg(a[0], a[1], a[2]) for a in r[1]]
        self._outputs_meta = [NodeArg(a[0], a[1], a[2]) for a in r[2]]
        self._overridable_initializers = [NodeArg(a[0], a[1], a[2]) for a in r[3]]
        self._model_meta = ModelMetadata(r[4])
        self._providers = ["CPUExecutionProvider"]

    def get_session_options(self):
        return self._sess_options

    def get_inputs(self):
        return list(self._inputs_meta)

    def get_outputs(self):
        return list(self._outputs_meta)

    def get_overridable_initializers(self):
        return list(self._overridable_initializers)

    def get_modelmeta(self):
        return self._model_meta

    def get_providers(self):
        return list(self._providers)

    def get_provider_options(self):
        return {"CPUExecutionProvider": {}}

    def set_providers(self, providers=None, provider_options=None):
        _normalize_providers(providers, provider_options, _o.providers())

    def disable_fallback(self):
        pass

    def enable_fallback(self):
        pass

    def _validate_input(self, feed_input_names):
        missing_input_names = []
        for i in self._inputs_meta:
            if i.name not in feed_input_names and not i.type.startswith("optional"):
                missing_input_names.append(i.name)
        if missing_input_names:
            raise ValueError("Required inputs (" + repr(missing_input_names) + ") are missing from input feed (" + repr(feed_input_names) + ").")

    def run(self, output_names, input_feed, run_options=None):
        self._validate_input(list(input_feed.keys()))
        if not output_names:
            output_names = [o.name for o in self._outputs_meta]
        types = {}
        for i in self._inputs_meta + self._overridable_initializers:
            types[i.name] = i.type
        feed = []
        for name, value in input_feed.items():
            if not isinstance(value, np.ndarray):
                t = _NP.get(types.get(name, ""))
                value = np.asarray(value) if t is None else np.asarray(value, dtype=t)
            feed.append((name, value))
        ro = run_options if run_options is not None else RunOptions()
        if not isinstance(ro, RunOptions):
            raise TypeError("run(): incompatible function arguments")
        try:
            return _o.run(self._id, list(output_names), feed, (int(ro.log_severity_level), int(ro.log_verbosity_level), ro.logid, bool(ro.terminate)))
        except RuntimeError as e:
            _raise(e)
