# os.path for native builds (src/dynpy/mod.rs): the POSIX path functions, and the file tests (exists, isfile,
# isdir, islink, getsize) through _brevi_fs, which holds --sandbox's read grant and folders.
import _brevi_fs
sep = "/"
curdir = "."
pardir = ".."
extsep = "."
altsep = None


def _s(p):
    if isinstance(p, str):
        return p
    return p.__fspath__()


def isabs(s):
    return _s(s).startswith("/")


def join(a, *p):
    path = _s(a)
    for b in p:
        b = _s(b)
        if b.startswith("/"):
            path = b
        elif path == "" or path.endswith("/"):
            path = path + b
        else:
            path = path + "/" + b
    return path


def split(p):
    p = _s(p)
    i = p.rfind("/") + 1
    head = p[:i]
    tail = p[i:]
    if head and head != "/" * len(head):
        head = head.rstrip("/")
    return (head, tail)


def basename(p):
    p = _s(p)
    return p[p.rfind("/") + 1:]


def dirname(p):
    p = _s(p)
    i = p.rfind("/") + 1
    head = p[:i]
    if head and head != "/" * len(head):
        head = head.rstrip("/")
    return head


def splitext(p):
    p = _s(p)
    dot = p.rfind(".")
    slash = p.rfind("/")
    if dot > slash:
        k = slash + 1
        while k < dot:
            if p[k] != ".":
                return (p[:dot], p[dot:])
            k += 1
    return (p, "")


def normcase(s):
    return _s(s)


def normpath(path):
    path = _s(path)
    if path == "":
        return "."
    initial = 0
    if path.startswith("/"):
        initial = 1
        if path.startswith("//") and not path.startswith("///"):
            initial = 2
    comps = path.split("/")
    new = []
    for c in comps:
        if c == "" or c == ".":
            continue
        if c != ".." or (initial == 0 and len(new) == 0) or (len(new) > 0 and new[-1] == ".."):
            new.append(c)
        elif len(new) > 0:
            new.pop()
    out = "/".join(new)
    if initial > 0:
        out = "/" * initial + out
    if out == "":
        return "."
    return out


def commonprefix(m):
    if len(m) == 0:
        return ""
    s1 = min(m)
    s2 = max(m)
    i = 0
    while i < len(s1):
        if s1[i] != s2[i]:
            return s1[:i]
        i += 1
    return s1


def _st(p, follow):
    return _brevi_fs.stat(_s(p), follow)


def exists(path):
    try:
        _st(path, True)
    except (OSError, ValueError):
        return False
    return True


def lexists(path):
    try:
        _st(path, False)
    except (OSError, ValueError):
        return False
    return True


def isfile(path):
    try:
        return _st(path, True)[0] == 1
    except (OSError, ValueError):
        return False


def isdir(s):
    try:
        return _st(s, True)[0] == 2
    except (OSError, ValueError):
        return False


def islink(path):
    try:
        return _st(path, False)[0] == 4
    except (OSError, ValueError):
        return False


def getsize(filename):
    return _st(filename, True)[1]
