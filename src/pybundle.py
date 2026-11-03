# Run by `brevi build --standalone` (see pybridge::bundle) with the python3 the program was built against:
#   python3 -c <this> SOURCE BINARY
# Appends this CPython to BINARY so the program runs on a machine without Python: libpython, the shared
# libraries it and the extension modules need (glibc aside), the standard library, and the installed
# packages the program imports with everything they require. pyrt.rs unpacks it on first run.
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

real = os.path.realpath
stdlib = real(sysconfig.get_paths()['stdlib'])
dynload = real(sysconfig.get_config_var('DESTSHARED') or os.path.join(stdlib, 'lib-dynload'))
ver = 'python%d.%d' % sys.version_info[:2]
platlib = getattr(sys, 'platlibdir', 'lib')
STD = '%s/%s' % (platlib, ver)
SITE = 'site-packages'
THIRD = ('site-packages', 'dist-packages')


def under(p, root):
    return p == root or p.startswith(root.rstrip('/') + '/')


def is_std(p):
    p = real(p)
    return (under(p, stdlib) or under(p, dynload)) and not any('/%s/' % t in p + '/' for t in THIRD)


files = {}  # path in bundle -> path here


def add(dest, path):
    files.setdefault(dest, path)


# The standard library, less what a program cannot use without a desktop or a compiler.
SKIP_STD = {'site-packages', 'dist-packages', 'test', 'tests', 'idle_test', 'idlelib', 'tkinter', 'turtledemo',
            'ensurepip', 'lib2to3', '__pycache__', '__phello__'}
for root in [stdlib] + ([dynload] if not under(dynload, stdlib) else []):
    base = STD if root == stdlib else STD + '/lib-dynload'
    for d, ds, fs in os.walk(root):
        ds[:] = [x for x in ds if x not in SKIP_STD and not x.startswith('config-')]
        for f in fs:
            if f.endswith(('.pyc', '.pyo', '.a', '.exe')) or f.startswith('_tkinter') or f == 'turtle.py':
                continue
            p = os.path.join(d, f)
            if os.path.isfile(p):
                add(base + '/' + os.path.relpath(p, root), p)

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
        add(SITE + '/' + os.path.relpath(p, r), p)
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
                    add(SITE + '/' + os.path.relpath(os.path.join(d, x), r), os.path.join(d, x))
    else:
        add(SITE + '/' + os.path.relpath(real(f), r), real(f))

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

conf = '\n'.join([libdest, 'app', STD, STD + '/lib-dynload', SITE]) + '\n'
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
