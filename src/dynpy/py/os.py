# os for native builds (src/dynpy/mod.rs): os.path, the path constants, and listing folders (listdir, walk)
# through _brevi_fs, which holds --sandbox's read grant and folders. Nothing that reaches processes or the
# environment.
import _brevi_fs
import posixpath as path

sep = "/"
curdir = "."
pardir = ".."
extsep = "."
altsep = None
linesep = "\n"
name = "posix"


def fspath(p):
    if isinstance(p, (str, bytes)):
        return p
    if hasattr(p, "__fspath__"):
        return p.__fspath__()
    raise TypeError("expected str, bytes or os.PathLike object, not " + type(p).__name__)


def listdir(path="."):
    return _brevi_fs.listdir(fspath(path))


def walk(top, topdown=True, onerror=None, followlinks=False):
    # CPython 3.13's os.walk: a folder's subfolders and files in the order the system lists them, the folders
    # (as the caller left them, top-down) walked in turn, symlinked folders listed but not entered by default
    top = fspath(top)
    try:
        names = _brevi_fs.listdir(top)
    except OSError as e:
        if onerror is not None:
            onerror(e)
        return
    dirs = []
    nondirs = []
    walk_dirs = []
    for n in names:
        full = path.join(top, n)
        try:
            is_dir = _brevi_fs.stat(full, True)[0] == 2
        except OSError:
            is_dir = False
        if is_dir:
            dirs.append(n)
            if not topdown:
                try:
                    link = _brevi_fs.stat(full, False)[0] == 4
                except OSError:
                    link = False
                if followlinks or not link:
                    walk_dirs.append(full)
        else:
            nondirs.append(n)
    if topdown:
        yield top, dirs, nondirs
        for d in dirs:
            new = path.join(top, d)
            if followlinks or not path.islink(new):
                yield from walk(new, topdown, onerror, followlinks)
    else:
        for new in walk_dirs:
            yield from walk(new, topdown, onerror, followlinks)
        yield top, dirs, nondirs
