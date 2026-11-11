# os.path on Windows for native builds (src/dynpy/mod.rs), and `import ntpath` everywhere: CPython 3.13's
# ntpath for the path text (splitroot, join, split, splitext, normpath, isabs with drive letters, UNC shares and
# both slashes), and the file tests (exists, isfile, isdir, islink, getsize) through _brevi_fs, which holds
# --sandbox's read grant and folders. Ported from Lib/ntpath.py and Lib/genericpath.py (PSF licence:
# https://docs.python.org/3/license.html).
import _brevi_fs
sep = "\\"
curdir = "."
pardir = ".."
extsep = "."
altsep = "/"
pathsep = ";"
devnull = "nul"


def _s(p):
    if isinstance(p, str):
        return p
    return p.__fspath__()


def normcase(s):
    return _s(s).replace("/", "\\").lower()


def isabs(s):
    s = _s(s)[:3].replace("/", "\\")
    # absolute: UNC and device paths, and paths with a drive and a root
    return s.startswith("\\\\") or s.startswith(":\\", 1)


def splitroot(p):
    p = _s(p)
    normp = p.replace("/", "\\")
    if normp[:1] == "\\":
        if normp[1:2] == "\\":
            # UNC drives (\\server\share, \\?\UNC\server\share) and device drives (\\.\device, \\?\device)
            start = 8 if normp[:8].upper() == "\\\\?\\UNC\\" else 2
            index = normp.find("\\", start)
            if index == -1:
                return (p, "", "")
            index2 = normp.find("\\", index + 1)
            if index2 == -1:
                return (p, "", "")
            return (p[:index2], p[index2:index2 + 1], p[index2 + 1:])
        # relative path with a root: \Windows
        return ("", p[:1], p[1:])
    if normp[1:2] == ":":
        if normp[2:3] == "\\":
            # absolute drive-letter path: X:\Windows
            return (p[:2], p[2:3], p[3:])
        # relative path with a drive: X:Windows
        return (p[:2], "", p[2:])
    return ("", "", p)


def splitdrive(p):
    drive, root, tail = splitroot(p)
    return (drive, root + tail)


def join(path, *paths):
    path = _s(path)
    result_drive, result_root, result_path = splitroot(path)
    for p in paths:
        p = _s(p)
        p_drive, p_root, p_path = splitroot(p)
        if p_root:
            # the next path is absolute
            if p_drive or not result_drive:
                result_drive = p_drive
            result_root = p_root
            result_path = p_path
            continue
        elif p_drive and p_drive != result_drive:
            if p_drive.lower() != result_drive.lower():
                # a different drive: the paths so far are dropped
                result_drive = p_drive
                result_root = p_root
                result_path = p_path
                continue
            # the same drive in another case
            result_drive = p_drive
        # the next path is relative to the result
        if result_path and result_path[-1] not in "\\/":
            result_path = result_path + "\\"
        result_path = result_path + p_path
    # a separator between a UNC drive and a relative path
    if result_path and not result_root and result_drive and result_drive[-1] not in ":\\/":
        return result_drive + "\\" + result_path
    return result_drive + result_root + result_path


def split(p):
    d, r, p = splitroot(p)
    i = len(p)
    while i and p[i - 1] not in "\\/":
        i -= 1
    head = p[:i]
    tail = p[i:]
    return (d + r + head.rstrip("\\/"), tail)


def basename(p):
    return split(p)[1]


def dirname(p):
    return split(p)[0]


def splitext(p):
    p = _s(p)
    sep_index = max(p.rfind("\\"), p.rfind("/"))
    dot = p.rfind(".")
    if dot > sep_index:
        k = sep_index + 1
        while k < dot:
            if p[k] != ".":
                return (p[:dot], p[dot:])
            k += 1
    return (p, "")


def normpath(path):
    path = _s(path).replace("/", "\\")
    drive, root, path = splitroot(path)
    prefix = drive + root
    comps = path.split("\\")
    out = []
    for c in comps:
        if c == "" or c == ".":
            continue
        if c == "..":
            if len(out) > 0 and out[-1] != "..":
                out.pop()
            elif len(out) == 0 and root:
                pass
            else:
                out.append(c)
        else:
            out.append(c)
    if not prefix and len(out) == 0:
        out.append(".")
    return prefix + "\\".join(out)


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
