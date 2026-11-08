# open() for native builds (src/dynpy/mod.rs): text and binary files, read whole when opened and written
# through to the file on each write, with CPython's modes, newline handling, errors and reprs. Files are reached
# only through _brevi_fs (src/dynpy/fsglue.rs), which holds --sandbox's read and write grants and folders.
# Not here (a program using them runs in CPython): '+' modes, seek and tell, buffering and opener.
import _brevi_fs


def _codec(encoding):
    e = "utf-8" if encoding is None else encoding.lower().replace("_", "-")
    if e in ("utf-8", "utf8", "u8"):
        return "utf-8"
    if e in ("ascii", "us-ascii"):
        return "ascii"
    if e in ("latin-1", "latin1", "iso-8859-1", "iso8859-1", "l1"):
        return "latin-1"
    raise LookupError("unknown encoding: " + encoding)


class _Base:
    def _check(self):
        if self.closed:
            raise ValueError("I/O operation on closed file.")

    def _check_read(self):
        self._check()
        if not self._reading:
            raise OSError("not readable")

    def _check_write(self):
        self._check()
        if self._reading:
            raise OSError("not writable")

    def readable(self):
        self._check()
        return self._reading

    def writable(self):
        self._check()
        return not self._reading

    def seekable(self):
        self._check()
        return False

    def flush(self):
        self._check()

    def close(self):
        self.closed = True

    def __enter__(self):
        self._check()
        return self

    def __exit__(self, *exc):
        self.close()
        return None

    def __iter__(self):
        self._check()
        return self

    def __next__(self):
        line = self.readline()
        if not line:
            raise StopIteration
        return line

    def read(self, size=-1):
        self._check_read()
        d, p = self._data, self._pos
        if size is None or size < 0:
            self._pos = len(d)
            return d[p:]
        self._pos = min(len(d), p + size)
        return d[p:self._pos]

    def readline(self, size=-1):
        self._check_read()
        d, p = self._data, self._pos
        i = d.find(self._nl, p)
        end = len(d) if i < 0 else i + 1
        if size is not None and size >= 0:
            end = min(end, p + size)
        self._pos = end
        return d[p:end]

    def readlines(self, hint=-1):
        out = []
        n = 0
        while True:
            line = self.readline()
            if not line:
                return out
            out.append(line)
            n += len(line)
            if hint is not None and 0 < hint <= n:
                return out

    def writelines(self, lines):
        for line in lines:
            self.write(line)

    def fileno(self):
        raise OSError("brevi: fileno() of a file is not native")


class TextIOWrapper(_Base):
    def __init__(self, name, mode, reading, encoding, errors, newline):
        self.name = name
        self.mode = mode
        self.closed = False
        self._reading = reading
        self._codec = _codec(encoding)
        self.encoding = "utf-8" if encoding is None else encoding
        self.errors = "strict" if errors is None else errors
        self._newline = newline
        self._nl = "\n"
        self._pos = 0
        self._data = ""
        if reading:
            text = _brevi_fs.load(name).decode(self._codec, self.errors)
            if newline is None:
                text = text.replace("\r\n", "\n").replace("\r", "\n")
            self._data = text

    def write(self, s):
        self._check_write()
        if not isinstance(s, str):
            raise TypeError("write() argument must be str, not " + type(s).__name__)
        t = s
        if self._newline not in (None, "", "\n"):
            t = t.replace("\n", self._newline)
        _brevi_fs.save(self.name, t.encode(self._codec, self.errors), 1)
        return len(s)

    def __repr__(self):
        return "<_io.TextIOWrapper name=%r mode=%r encoding=%r>" % (self.name, self.mode, self.encoding)


class BufferedFile(_Base):
    def __init__(self, name, mode, reading):
        self.name = name
        self.mode = mode
        self.closed = False
        self._reading = reading
        self._nl = b"\n"
        self._pos = 0
        self._data = _brevi_fs.load(name) if reading else b""

    def write(self, b):
        self._check_write()
        if isinstance(b, str):
            raise TypeError("a bytes-like object is required, not 'str'")
        _brevi_fs.save(self.name, bytes(b), 1)
        return len(b)

    def __repr__(self):
        kind = "BufferedReader" if self._reading else "BufferedWriter"
        return "<_io.%s name=%r>" % (kind, self.name)


def open(file, mode="r", buffering=-1, encoding=None, errors=None, newline=None, closefd=True, opener=None):
    if not isinstance(file, str):
        if hasattr(file, "__fspath__"):
            file = file.__fspath__()
        elif isinstance(file, int):
            raise OSError("brevi: open() of a file descriptor is not native")
        else:
            raise TypeError("expected str, bytes or os.PathLike object, not " + type(file).__name__)
    if not isinstance(mode, str):
        raise TypeError("open() argument 'mode' must be str, not " + type(mode).__name__)
    if opener is not None:
        raise OSError("brevi: open(opener=...) is not native")
    if len(set(mode)) != len(mode) or any(c not in "rwxabt+" for c in mode):
        raise ValueError("invalid mode: '%s'" % mode)
    kinds = [c for c in mode if c in "rwxa"]
    if len(kinds) != 1:
        raise ValueError("must have exactly one of create/read/write/append mode")
    binary = "b" in mode
    if binary and "t" in mode:
        raise ValueError("can't have text and binary mode at once")
    if "+" in mode:
        raise OSError("brevi: open() mode '%s' (reading and writing) is not native" % mode)
    if binary and encoding is not None:
        raise ValueError("binary mode doesn't take an encoding argument")
    if binary and errors is not None:
        raise ValueError("binary mode doesn't take an errors argument")
    if binary and newline is not None:
        raise ValueError("binary mode doesn't take a newline argument")
    if newline not in (None, "", "\n", "\r", "\r\n"):
        raise ValueError("illegal newline value: %s" % (newline,))
    k = kinds[0]
    if k != "r":
        _brevi_fs.save(file, b"", {"w": 0, "a": 1, "x": 2}[k])
    if not binary:
        _codec(encoding)
        return TextIOWrapper(file, mode, k == "r", encoding, errors, newline)
    return BufferedFile(file, mode, k == "r")
