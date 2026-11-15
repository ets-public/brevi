# Run by `brevi build --standalone` (see pybridge::bundle) with the python3 the program was built against:
#   python3 -c <this> SOURCE BINARY
# Appends this CPython to BINARY so the program runs on a machine without Python: libpython, the shared
# libraries it and the extension modules need (glibc aside; on Windows, Windows' own DLLs aside), the standard
# library, and the installed packages the program imports with everything they require. pyrt_load.rs unpacks
# it on first run. On Windows the layout is a python.org install's (python313.dll, Lib, DLLs,
# Lib/site-packages), so CPython's own Windows start-up finds everything as it does there.
#
# Layout of what is appended:
#   entries, each: kind u8 (0 deflate, 1 stored), mode u32, mtime u64, path len u16, path, size u32,
#                  data len u32, data
#   trailer (32 bytes): entries length u64, tag (16 hex chars), b"BRVPYB01"
# A bundle holds two files of its own: .brevi-conf (libpython, then sys.path, relative, one per line) and
# .brevi-preload (shared libraries to load first, in an order that works).
import hashlib, importlib, importlib.metadata as md, io, os, re, struct, subprocess, sys, sysconfig, zlib

src, binary = sys.argv[1], sys.argv[2]
# Loaded by python3's own start-up (site, .pth files such as setuptools' _distutils_hack), not by the program.
before = set(sys.modules)
sys.path[:] = [p for p in sys.path if p not in ('', '.', os.getcwd())]
lib = sys.argv[3] if len(sys.argv) > 3 else ''

WIN = sys.platform == 'win32'
real = os.path.realpath
stdlib = real(sysconfig.get_paths()['stdlib'])
ver = 'python%d.%d' % sys.version_info[:2]
platlib = getattr(sys, 'platlibdir', 'lib')
if WIN:
    # A python.org (or uv) install: python3XX.dll and vcruntime beside python.exe, extension modules in DLLs.
    dynload = real(os.path.join(sys.base_prefix, 'DLLs'))
    STD, DYN, SITE = 'Lib', 'DLLs', 'Lib/site-packages'
else:
    dynload = real(sysconfig.get_config_var('DESTSHARED') or os.path.join(stdlib, 'lib-dynload'))
    STD = '%s/%s' % (platlib, ver)
    DYN, SITE = STD + '/lib-dynload', 'site-packages'
THIRD = ('site-packages', 'dist-packages')


def fold(p):
    return os.path.normcase(p)


def under(p, root):
    p, root = fold(p), fold(root)
    return p == root or p.startswith(root.rstrip(os.sep) + os.sep)


def rel(p, root):
    return os.path.relpath(p, root).replace(os.sep, '/')


def is_std(p):
    p = real(p)
    return (under(p, stdlib) or under(p, dynload)) and not any(os.sep + t + os.sep in fold(p) + os.sep for t in THIRD)


def refuse(why):
    print('brevi: refused: ' + why, file=sys.stderr)
    sys.exit(2)


if WIN and '\\windowsapps\\' in fold(real(sys.base_prefix)) + '\\':
    refuse('this is the Microsoft Store Python, whose files cannot be bundled: install CPython 3.13 from python.org (or with uv) and build with that')


files = {}  # path in bundle -> path here


def add(dest, path):
    files.setdefault(dest, path)


# The standard library, less what a program cannot use without a desktop or a compiler.
SKIP_STD = {'site-packages', 'dist-packages', 'test', 'tests', 'idle_test', 'idlelib', 'tkinter', 'turtledemo',
            'ensurepip', 'lib2to3', '__pycache__', '__phello__'}
for root in [stdlib] + ([dynload] if not under(dynload, stdlib) else []):
    base = STD if root == stdlib else DYN
    for d, ds, fs in os.walk(root):
        ds[:] = [x for x in ds if x not in SKIP_STD and not x.startswith('config-')]
        for f in fs:
            if f.endswith(('.pyc', '.pyo', '.a', '.exe', '.pdb', '.ico')) or f.startswith('_tkinter') or f == 'turtle.py':
                continue
            if WIN and re.match(r'(tcl|tk)\d+t?\.dll$', f, re.I):
                continue
            p = os.path.join(d, f)
            if os.path.isfile(p):
                add(base + '/' + rel(p, root), p)

# The program's imports, and every installed (non standard) module importing them loads.
text = open(src, encoding='utf-8', errors='replace').read()
names = set()
for m in re.finditer(r'^[ \t]*import[ \t]+([^\n#;]+)', text, re.M):
    for part in m.group(1).split(','):
        w = part.strip().split()
        if w:
            names.add(w[0].split('.')[0])
names.update(re.findall(r'^[ \t]*from[ \t]+([A-Za-z_]\w*)', text, re.M))
srcdir = real(os.path.dirname(os.path.abspath(src)))
for n in sorted(names):
    if n in sys.builtin_module_names or n in sys.modules and is_std(getattr(sys.modules[n], '__file__', None) or stdlib):
        continue
    try:
        spec = importlib.util.find_spec(n)
    except BaseException:
        spec = None
    origin = spec and (spec.origin or (list(spec.submodule_search_locations or []) or [None])[0])
    if not origin or origin in ('built-in', 'frozen') or is_std(origin) or under(real(origin), srcdir):
        continue
    try:
        importlib.import_module(n)
    except BaseException as e:
        print('brevi: %s: import failed while bundling (%s)' % (n, e), file=sys.stderr)

roots = sorted({real(p) for p in sys.path if p and os.path.isdir(p)}, key=len, reverse=True)


def root_of(p):
    p = real(p)
    return next((r for r in roots if under(p, r) and r != p), None)


try:
    by_top = md.packages_distributions()
except Exception:
    by_top = {}
dists, tops = set(), set()
for name, mod in list(sys.modules.items()):
    if name in before:
        continue
    f = getattr(mod, '__file__', None)
    if not f or is_std(f) or under(real(f), srcdir):
        continue
    top = name.split('.')[0]
    ds = by_top.get(top, [])
    dists.update(ds)
    if not ds:
        tops.add((top, f))


def norm(n):
    return re.sub(r'[-_.]+', '-', n).lower()


try:
    from packaging.markers import Marker
except Exception:
    Marker = None
seen, todo = set(), list(dists)
while todo:
    d = todo.pop()
    if norm(d) in seen:
        continue
    seen.add(norm(d))
    try:
        dist = md.distribution(d)
    except Exception:
        continue
    for r in dist.requires or []:
        req, _, marker = r.partition(';')
        if 'extra' in marker:
            continue
        if marker.strip() and Marker:
            try:
                if not Marker(marker.strip()).evaluate():
                    continue
            except Exception:
                pass
        m = re.match(r'\s*([A-Za-z0-9][A-Za-z0-9._-]*)', req)
        if m:
            todo.append(m.group(1))
    found = False
    for f in dist.files or []:
        p = real(str(dist.locate_file(f)))
        r = root_of(p)
        if not r or '__pycache__' in p or p.endswith('.pyc') or not os.path.isfile(p) or is_std(p):
            continue
        found = True
        add(SITE + '/' + rel(p, r), p)
    if not found:
        for t in (dist.read_text('top_level.txt') or '').split():
            spec = importlib.util.find_spec(t)
            if spec and spec.origin:
                tops.add((t, spec.origin))
for top, f in tops:
    r = root_of(f)
    if not r:
        continue
    p = os.path.join(r, top)
    if os.path.isdir(p):
        for d, ds, fs in os.walk(p):
            ds[:] = [x for x in ds if x != '__pycache__']
            for x in fs:
                if not x.endswith('.pyc'):
                    add(SITE + '/' + rel(os.path.join(d, x), r), os.path.join(d, x))
    else:
        add(SITE + '/' + rel(real(f), r), real(f))

if WIN:
    # Windows has no ldd: each bundled .pyd and .dll's own import tables (PE import and delay-import
    # directories) say which DLLs it needs, and this process's loaded modules say which ones the program's
    # imports really loaded. A DLL that is part of Windows (System32, the API sets) is left to the system; any
    # other is bundled, the Visual C++ runtime included (it is not part of Windows). One found beside
    # python.exe (vcruntime140.dll, python3.dll) or nowhere in the bundle goes beside python3XX.dll and is
    # loaded first by its full path, so later loads of that name find it, as Windows reuses a loaded module of
    # the same name.
    import ctypes
    k32 = ctypes.WinDLL('kernel32', use_last_error=True)
    buf = ctypes.create_unicode_buffer(32768)
    k32.GetSystemDirectoryW(buf, 32768)
    sysdir = real(buf.value)
    k32.GetWindowsDirectoryW(buf, 32768)
    windir = real(buf.value)
    REDIST = re.compile(r'^(vcruntime|msvcp|concrt|vccorlib|vcomp|vcamp|mfc)\d.*\.dll$', re.I)

    def pe(path):
        # (machine, [imported DLL names]) of a PE file, None if it is not one.
        try:
            with open(path, 'rb') as fh:
                d = fh.read()
            if d[:2] != b'MZ':
                return None
            o = struct.unpack_from('<I', d, 0x3c)[0]
            if d[o:o + 4] != b'PE\0\0':
                return None
            machine, nsec = struct.unpack_from('<HH', d, o + 4)
            optsize = struct.unpack_from('<H', d, o + 20)[0]
            opt = o + 24
            if struct.unpack_from('<H', d, opt)[0] == 0x20b:
                base, ndd, dd = struct.unpack_from('<Q', d, opt + 24)[0], struct.unpack_from('<I', d, opt + 108)[0], opt + 112
            else:
                base, ndd, dd = struct.unpack_from('<I', d, opt + 28)[0], struct.unpack_from('<I', d, opt + 92)[0], opt + 96
            secs = [struct.unpack_from('<IIII', d, opt + optsize + 40 * i + 8) for i in range(nsec)]

            def off(rva):
                for vsize, va, rawsize, rawptr in secs:
                    if va <= rva < va + max(vsize, rawsize):
                        return rva - va + rawptr
                return None

            def name(rva):
                p = off(rva)
                return None if p is None else d[p:d.index(b'\0', p)].decode('ascii', 'replace')

            got = []
            if ndd > 1:
                rva = struct.unpack_from('<I', d, dd + 8)[0]
                p = off(rva) if rva else None
                while p is not None and p + 20 <= len(d):
                    oft, _, _, nrva, ft = struct.unpack_from('<IIIII', d, p)
                    if not (oft or nrva or ft):
                        break
                    n = name(nrva)
                    if n:
                        got.append(n)
                    p += 20
            if ndd > 13:
                rva = struct.unpack_from('<I', d, dd + 13 * 8)[0]
                p = off(rva) if rva else None
                while p is not None and p + 32 <= len(d):
                    attrs, nrva = struct.unpack_from('<II', d, p)
                    if not nrva:
                        break
                    n = name(nrva if attrs & 1 else nrva - base)
                    if n:
                        got.append(n)
                    p += 32
            return machine, got
        except (OSError, struct.error, ValueError):
            return None

    MACHINE = {0x14c: 'x86', 0x8664: 'x64', 0xaa64: 'ARM64'}
    lm, bm = (pe(lib) or (None,))[0], (pe(binary) or (None,))[0]
    if lm != bm:
        refuse('%s is a %s DLL but the program is a %s binary: build with a CPython 3.13 for the same CPU' % (os.path.basename(lib), MACHINE.get(lm, lm), MACHINE.get(bm, bm)))
    libdest = os.path.basename(lib)
    add(libdest, real(lib))
    order = []

    def os_dll(p):
        return under(p, windir) and not REDIST.match(os.path.basename(p))

    # sys.path on Windows holds python.exe's own folder too: a DLL there (vcruntime140.dll) is not a package's.
    homes = {fold(real(sys.base_prefix)), fold(real(sys.prefix))}
    places = sorted({r for r in roots + [dynload, stdlib] if fold(r) not in homes}, key=len, reverse=True)

    def place(p):
        # Where a DLL found here goes in the bundle: its own place under the standard library or a package
        # folder, else beside python3XX.dll (and loaded first).
        for r in places:
            if under(p, r):
                base = DYN if r == dynload else STD if r == stdlib else SITE
                return base + '/' + rel(p, r), False
        return os.path.basename(p), True

    have = {fold(p) for p in files.values()}
    dllnames = {os.path.basename(d).lower() for d in files}
    todo = [p for d, p in sorted(files.items()) if d.lower().endswith(('.pyd', '.dll'))]

    def take(p):
        p = real(p)
        if fold(p) in have or os_dll(p) or p.lower().endswith('.exe'):
            return
        dest, first = place(p)
        if dest.lower() in (x.lower() for x in files):
            return
        add(dest, p)
        have.add(fold(p))
        dllnames.add(os.path.basename(p).lower())
        if first:
            order.append(dest)
        todo.append(p)

    # What importing the program's modules loaded into this process.
    try:
        import ctypes.wintypes as wt
        psapi = ctypes.WinDLL('psapi')
        k32.GetCurrentProcess.restype = wt.HANDLE
        proc = k32.GetCurrentProcess()
        psapi.EnumProcessModules.argtypes = [wt.HANDLE, ctypes.c_void_p, wt.DWORD, ctypes.POINTER(wt.DWORD)]
        k32.GetModuleFileNameW.argtypes = [wt.HMODULE, wt.LPWSTR, wt.DWORD]
        got = wt.DWORD(1024 * ctypes.sizeof(wt.HMODULE))
        while True:
            hs = (wt.HMODULE * (got.value // ctypes.sizeof(wt.HMODULE) + 64))()
            if not psapi.EnumProcessModules(proc, hs, ctypes.sizeof(hs), ctypes.byref(got)):
                raise ctypes.WinError(ctypes.get_last_error())
            if got.value <= ctypes.sizeof(hs):
                break
        loaded = [buf.value for h in hs[:got.value // ctypes.sizeof(wt.HMODULE)] if h and k32.GetModuleFileNameW(h, buf, 32768)]
    except Exception as e:
        refuse('cannot list the DLLs this Python loaded (%s)' % e)
    for p in loaded:
        take(p)
    seen_pe = set()
    while todo:
        p = todo.pop()
        if fold(p) in seen_pe:
            continue
        seen_pe.add(fold(p))
        info = pe(p)
        if not info:
            continue
        for n in info[1]:
            lo = n.lower()
            if lo.startswith(('api-ms-', 'ext-ms-')) or lo in dllnames:
                continue
            hit = next((c for c in (os.path.join(os.path.dirname(p), n), os.path.join(sys.base_prefix, n), os.path.join(sysdir, n)) if os.path.isfile(c)), None)
            if hit:
                take(hit)
            else:
                print('brevi: %s (needed by %s) is not on this machine, so that file cannot load from the bundle either' % (n, os.path.basename(p)), file=sys.stderr)
    conf = '\n'.join([libdest, 'app', STD, DYN, SITE]) + '\n'
    pre = ''.join(d + '\n' for d in order)
else:
    # libpython, and the shared libraries everything needs that are not part of the C library itself (which
    # every Linux has, and which must match the system's own loader).
    GLIBC = re.compile(r'^(ld-linux.*|ld64.*|linux-vdso.*|linux-gate.*|lib(c|m|mvec|pthread|dl|rt|util|resolv|nsl|anl|BrokenLocale|thread_db)\.so.*|libnss_.*)$')
    libdest = '%s/%s' % (platlib, os.path.basename(lib))
    add(libdest, real(lib))
    deps, order = {}, []
    inside = [stdlib, dynload] + roots


    def need(path):
        try:
            out = subprocess.run(['ldd', path], capture_output=True, text=True).stdout
        except Exception:
            return []
        got = []
        for line in out.splitlines():
            m = re.match(r'\s*(\S+)\s*=>\s*(/\S+)', line)
            if not m or GLIBC.match(m.group(1)) or real(m.group(2)) == real(lib):
                continue
            p = real(m.group(2))
            if any(under(p, r) for r in inside if r != '/'):
                continue
            got.append((m.group(1), p))
        return got


    for dest, p in sorted(files.items()):
        if re.search(r'\.so(\.|$)', dest):
            for so, p2 in need(p):
                if so not in deps:
                    deps[so] = p2
                    order.append(so)
    for so in list(order):
        for so2, p2 in need(deps[so]):
            if so2 not in deps:
                deps[so2] = p2
                order.append(so2)
    for so in order:
        add('%s/brevi-deps/%s' % (platlib, so), deps[so])

    conf = '\n'.join([libdest, 'app', STD, DYN, SITE]) + '\n'
    pre = ''.join('%s/brevi-deps/%s\n' % (platlib, so) for so in order)

# Write the binary again (never in place: it may share its file with Brevi's build cache) with the bundle.
body = io.BytesIO()


def entry(path, data, mode, mtime):
    raw = len(data)
    c = zlib.compressobj(6, zlib.DEFLATED, -15)
    z = c.compress(data) + c.flush()
    kind = 0
    if len(z) >= raw:
        z, kind = data, 1
    pb = path.encode()
    body.write(struct.pack('<BIQH', kind, mode, mtime, len(pb)) + pb + struct.pack('<II', raw, len(z)))
    body.write(z)


h = hashlib.sha256()
for dest in sorted(files):
    p = files[dest]
    st = os.stat(p)
    with open(p, 'rb') as fh:
        data = fh.read()
    h.update(dest.encode() + b'\0' + data)
    entry(dest, data, st.st_mode & 0o777, int(st.st_mtime))
for name, txt in (('.brevi-conf', conf), ('.brevi-preload', pre)):
    h.update(name.encode() + b'\0' + txt.encode())
    entry(name, txt.encode(), 0o644, 0)
tag = h.hexdigest()[:16]
data = body.getvalue()
tmp = binary + '.brevi-tmp'
with open(binary, 'rb') as fh:
    exe = fh.read()
with open(tmp, 'wb') as fh:
    fh.write(exe)
    fh.write(data)
    fh.write(struct.pack('<Q', len(data)) + tag.encode() + b'BRVPYB01')
os.chmod(tmp, 0o755)
os.replace(tmp, binary)
print('%d %d %d' % (len(files), len(data), len(order)))
