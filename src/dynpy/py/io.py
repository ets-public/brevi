# io.StringIO for native builds (src/dynpy/mod.rs): an in-memory text file, as CPython's. Only StringIO
# (a program using more of io runs in CPython, or is refused under the sandbox). Newlines are kept as written.


class StringIO:
    def __init__(self, initial_value="", newline="\n"):
        if initial_value is None:
            initial_value = ""
        if not isinstance(initial_value, str):
            raise TypeError("initial_value must be str or None, not " + type(initial_value).__name__)
        self._buf = initial_value
        self._pos = 0
        self.closed = False

    def _check(self):
        if self.closed:
            raise ValueError("I/O operation on closed file")

    def getvalue(self):
        self._check()
        return self._buf

    def write(self, s):
        self._check()
        if not isinstance(s, str):
            raise TypeError("string argument expected, got '" + type(s).__name__ + "'")
        p = self._pos
        if p > len(self._buf):
            self._buf = self._buf + "\0" * (p - len(self._buf))
        self._buf = self._buf[:p] + s + self._buf[p + len(s):]
        self._pos = p + len(s)
        return len(s)

    def writelines(self, lines):
        for s in lines:
            self.write(s)

    def read(self, size=-1):
        self._check()
        p = self._pos
        if size is None or size < 0:
            end = len(self._buf)
        else:
            end = min(len(self._buf), p + size)
        if p >= end:
            return ""
        self._pos = end
        return self._buf[p:end]

    def readline(self, size=-1):
        self._check()
        p = self._pos
        if p >= len(self._buf):
            return ""
        i = self._buf.find("\n", p)
        end = len(self._buf) if i < 0 else i + 1
        if size is not None and size >= 0:
            end = min(end, p + size)
        self._pos = end
        return self._buf[p:end]

    def readlines(self, hint=-1):
        out = []
        while True:
            line = self.readline()
            if not line:
                return out
            out.append(line)

    def seek(self, pos, whence=0):
        self._check()
        if whence == 0:
            if pos < 0:
                raise ValueError("Negative seek position " + str(pos))
            self._pos = pos
        elif whence == 1:
            if pos != 0:
                raise OSError("Can't do nonzero cur-relative seeks")
        elif whence == 2:
            if pos != 0:
                raise OSError("Can't do nonzero end-relative seeks")
            self._pos = len(self._buf)
        else:
            raise ValueError("Invalid whence (" + str(whence) + ", should be 0, 1 or 2)")
        return self._pos

    def tell(self):
        self._check()
        return self._pos

    def truncate(self, size=None):
        self._check()
        if size is None:
            size = self._pos
        if size < 0:
            raise ValueError("Negative size value " + str(size))
        self._buf = self._buf[:size]
        return size

    def readable(self):
        return True

    def writable(self):
        return True

    def seekable(self):
        return True

    def flush(self):
        self._check()

    def close(self):
        self.closed = True

    def __iter__(self):
        self._check()
        return self

    def __next__(self):
        line = self.readline()
        if not line:
            raise StopIteration
        return line

    def __enter__(self):
        return self

    def __exit__(self, *a):
        self.close()
        return False
