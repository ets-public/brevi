# Brevi's safetensors: the safetensors 0.8 package with numpy (framework "np" / "numpy"), over the format's port
# in _brevi_st (headers read as the Rust crate reads them, with its errors; slicing; serialization) and files
# through _brevi_fs (read and write grants under --sandbox). safe_open reads the header when it opens a file and
# each tensor's bytes when asked for it, as the package's pread backend does.
# Not here (caps.rs SAFETENSORS_LACKS sends a program using them to CPython's safetensors): the torch, tensorflow,
# flax, mlx and paddle frameworks, TensorSpec with serialize / serialize_file, deserialize (its bytearrays),
# devices other than the CPU.
# Tensors of dtypes Brevi's numpy lacks (F16, BF16, U8, I8, ...) raise NotImplementedError when they are read.
import _brevi_fs as _fs
import _brevi_st as _st

__version__ = "0.8.0"

_FRAMEWORKS = {"pt": "pytorch", "torch": "pytorch", "pytorch": "pytorch", "tf": "tensorflow", "tensorflow": "tensorflow",
               "jax": "flax", "flax": "flax", "mlx": "mlx", "np": "numpy", "numpy": "numpy", "paddle": "paddle"}


class SafetensorError(Exception):
    pass


def _err(e):
    return SafetensorError(str(e))


def _open_path(filename):
    if hasattr(filename, "__fspath__"):
        return filename.__fspath__()
    if isinstance(filename, str):
        return filename
    raise TypeError("argument 'filename': '" + type(filename).__name__ + "' object cannot be converted to 'PyString'")


def _index(it):
    # numpy integer scalars index as ints (the binding takes anything with __index__)
    if isinstance(it, (int, slice)) or it is Ellipsis:
        return it
    if type(it).__name__ in ("int64", "int32", "int16", "int8", "uint64", "uint32", "uint16", "uint8", "intp"):
        return int(it)
    return it


class _Slice:
    def __init__(self, f, name, info):
        self._f = f
        self._name = name
        self._info = info

    def get_shape(self):
        return list(self._info[2])

    def get_dtype(self):
        return _st.dtype_name(self._info[1])

    def __getitem__(self, idx):
        shape = self._info[2]
        if isinstance(idx, tuple):
            items = list(idx)
        elif isinstance(idx, list):
            if len(idx) == 0:
                return self._f._slice(self._info, [(False, 0, 0, 1)], "[0:0]")
            raise SafetensorError("Non empty lists are not implemented")
        else:
            items = [idx]
        items = [_index(it) for it in items]
        n_ell = 0
        for it in items:
            if it is Ellipsis:
                n_ell += 1
        if n_ell > 1:
            raise SafetensorError("Only one ellipsis (...) is allowed in slice index")
        n_exp = len(shape) - (len(items) - n_ell)
        if n_exp < 0:
            n_exp = 0
        out = []
        shown = []
        dim = 0
        for it in items:
            if it is Ellipsis:
                for _ in range(n_exp):
                    out.append((False, -1, -1, 1))
                    shown.append(":")
                    dim += 1
            elif isinstance(it, slice):
                start = it.start
                stop = it.stop
                for v in (start, stop):
                    if v is not None and v < 0:
                        raise OverflowError("can't convert negative int to unsigned")
                step = 1 if it.step is None else it.step
                if step <= 0:
                    raise SafetensorError("Slice step must be a positive integer, got " + str(step))
                a = "" if start is None else str(start)
                b = "" if stop is None else str(stop)
                shown.append(a + ":" + b if step == 1 else a + ":" + b + ":" + str(step))
                out.append((False, -1 if start is None else int(start), -1 if stop is None else int(stop), int(step)))
                dim += 1
            elif isinstance(it, int) and -2147483648 <= it <= 2147483647:
                d = shape[dim] if dim < len(shape) else 0
                i = int(it)
                if i < 0:
                    if d + i < 0:
                        raise SafetensorError("Invalid index " + str(i) + " for dimension " + str(dim) + " of size " + str(d))
                    i = d + i
                out.append((True, i, i + 1, 1))
                shown.append(str(i))
                dim += 1
            else:
                raise SafetensorError("Unsupported slice index at position " + str(dim) + ": expected slice, int, or ellipsis")
        return self._f._slice(self._info, out, "[" + ", ".join(shown) + "]")


class safe_open:
    def __init__(self, filename, framework, device="cpu", backend="mmap"):
        path = _open_path(filename)
        fw = _FRAMEWORKS.get(framework) if isinstance(framework, str) else None
        if fw is None:
            raise SafetensorError("framework " + str(framework) + " is invalid")
        if fw != "numpy":
            raise NotImplementedError("brevi: Brevi's safetensors reads numpy arrays (framework='np'), not " + fw)
        if device not in ("cpu", None):
            raise SafetensorError("Device " + str(device) + " is not supported for framework " + fw)
        if backend not in ("mmap", "pread"):
            raise SafetensorError('backend "' + str(backend) + '" is invalid (expected one of: "mmap", "pread")')
        try:
            total = _fs.size(path)
            head = _fs.read_at(path, 0, 8)
        except PermissionError:
            raise
        except OSError:
            raise FileNotFoundError("No such file or directory: " + path)
        try:
            n = _st.header_len(head)
            head = _fs.read_at(path, 0, 8 + n)
            hdr = _st.header(head, total)
        except ValueError as e:
            raise SafetensorError("Error while deserializing header: " + str(e))
        self._path = path
        self._offset = hdr[0] + 8
        self._meta = hdr[1]
        self._tensors = hdr[2]
        self._index = {}
        for t in self._tensors:
            self._index[t[0]] = t
        self._open = True

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self._open = False
        return False

    def _check(self):
        if not self._open:
            raise SafetensorError("File is closed")

    def _info(self, name):
        self._check()
        t = self._index.get(name)
        if t is None:
            raise SafetensorError("File does not contain tensor " + str(name))
        return t

    def _bytes(self, t):
        n = t[4] - t[3]
        if n == 0:
            return b""
        return _fs.read_at(self._path, self._offset + t[3], n)

    def _slice(self, t, ix, shown):
        try:
            return _st.slice(self._bytes(t), t[1], t[2], ix, shown)
        except ValueError as e:
            raise SafetensorError(str(e))

    def keys(self):
        self._check()
        return sorted(self._index.keys())

    def offset_keys(self):
        self._check()
        return [t[0] for t in self._tensors]

    def metadata(self):
        self._check()
        if self._meta is None:
            return None
        return dict(self._meta)

    def get_tensor(self, name):
        t = self._info(name)
        return _st.tensor(self._bytes(t), t[1], t[2])

    def get_tensors(self):
        self._check()
        out = {}
        for t in self._tensors:
            out[t[0]] = _st.tensor(self._bytes(t), t[1], t[2])
        return out

    def get_slice(self, name):
        return _Slice(self, name, self._info(name))

