# Brevi's onnxruntime.capi.onnxruntime_pybind11_state: the exceptions onnxruntime's binding raises (one per
# ONNX Runtime status code). Its NotImplemented is not here (the name is Python's constant).


class Fail(Exception):
    pass


class InvalidArgument(Exception):
    pass


class NoSuchFile(Exception):
    pass


class NoModel(Exception):
    pass


class EngineError(Exception):
    pass


class RuntimeException(Exception):
    pass


class InvalidProtobuf(Exception):
    pass


class ModelLoaded(Exception):
    pass


class NotImplementedStatus(Exception):
    pass


class InvalidGraph(Exception):
    pass


class EPFail(Exception):
    pass


class ModelLoadCanceled(Exception):
    pass


class ModelRequiresCompilation(Exception):
    pass


class NotFound(Exception):
    pass
