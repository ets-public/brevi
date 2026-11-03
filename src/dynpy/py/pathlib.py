# pathlib for sandboxed builds (src/dynpy/mod.rs): PurePath / PurePosixPath only, no file access.
class PurePosixPath:
    def __init__(self, *args):
        parts = []
        for a in args:
            if isinstance(a, PurePosixPath):
                s = a._str
            elif isinstance(a, str):
                s = a
            else:
                raise TypeError("argument should be a str or an os.PathLike object, not " + repr(type(a).__name__))
            if s.startswith("/"):
                parts = [s]
            elif s != "":
                parts.append(s)
        root = ""
        rest = []
        for i in range(len(parts)):
            s = parts[i]
            if i == 0 and s.startswith("/"):
                root = "/"
                if s.startswith("//") and not s.startswith("///"):
                    root = "//"
            for c in s.split("/"):
                if c != "" and c != ".":
                    rest.append(c)
        self._root = root
        self._tail = rest
        if root == "" and len(rest) == 0:
            self._str = "."
        else:
            self._str = root + "/".join(rest)

    def __str__(self):
        return self._str

    def __fspath__(self):
        return self._str

    def __repr__(self):
        return "PurePosixPath(" + repr(self._str) + ")"

    def __eq__(self, other):
        if not isinstance(other, PurePosixPath):
            return False
        return self._str == other._str

    def __ne__(self, other):
        return not self.__eq__(other)

    def __hash__(self):
        return hash(self._str)

    def __lt__(self, other):
        return self._tail_key() < other._tail_key()

    def __le__(self, other):
        return self._tail_key() <= other._tail_key()

    def __gt__(self, other):
        return self._tail_key() > other._tail_key()

    def __ge__(self, other):
        return self._tail_key() >= other._tail_key()

    def _tail_key(self):
        return [self._root] + self._tail

    def __truediv__(self, other):
        return PurePosixPath(self, other)

    def __rtruediv__(self, other):
        return PurePosixPath(other, self)

    def joinpath(self, *others):
        return PurePosixPath(self, *others)

    def as_posix(self):
        return self._str

    def is_absolute(self):
        return self._root != ""

    @property
    def drive(self):
        return ""

    @property
    def root(self):
        return self._root

    @property
    def anchor(self):
        return self._root

    @property
    def parts(self):
        if self._root != "":
            return tuple([self._root] + self._tail)
        return tuple(self._tail)

    @property
    def name(self):
        if len(self._tail) == 0:
            return ""
        return self._tail[-1]

    @property
    def suffix(self):
        n = self.name
        i = n.rfind(".")
        if 0 < i < len(n) - 1:
            return n[i:]
        return ""

    @property
    def suffixes(self):
        n = self.name
        if n.endswith("."):
            return []
        n = n.lstrip(".")
        return ["." + s for s in n.split(".")[1:]]

    @property
    def stem(self):
        n = self.name
        i = n.rfind(".")
        if 0 < i < len(n) - 1:
            return n[:i]
        return n

    @property
    def parent(self):
        if len(self._tail) == 0:
            return self
        return PurePosixPath(self._root + "/".join(self._tail[:-1]))

    @property
    def parents(self):
        out = []
        p = self
        while len(p._tail) > 0:
            p = p.parent
            out.append(p)
        return tuple(out)

    def with_name(self, name):
        if len(self._tail) == 0:
            raise ValueError(repr(self) + " has an empty name")
        if name == "" or "/" in name or name == ".":
            raise ValueError("Invalid name " + repr(name))
        return PurePosixPath(self._root + "/".join(self._tail[:-1] + [name]))

    def with_stem(self, stem):
        return self.with_name(stem + self.suffix)

    def with_suffix(self, suffix):
        if suffix != "" and (not suffix.startswith(".") or suffix == "." or "/" in suffix):
            raise ValueError("Invalid suffix " + repr(suffix))
        if self.name == "":
            raise ValueError(repr(self) + " has an empty name")
        return self.with_name(self.stem + suffix)

    def is_relative_to(self, other):
        o = PurePosixPath(other)
        return o._root == self._root and self._tail[:len(o._tail)] == o._tail

    def relative_to(self, other):
        o = PurePosixPath(other)
        if not self.is_relative_to(o):
            raise ValueError(repr(self._str) + " is not in the subpath of " + repr(o._str))
        return PurePosixPath("/".join(self._tail[len(o._tail):]))


PurePath = PurePosixPath
