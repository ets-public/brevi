"""Writes src/dynpy/interned_313.txt: the name-like strings ([A-Za-z0-9_], 2 or more characters) that CPython
3.13.13 has interned when it compiles a script's main module, started as tools/conformance.py starts it.

Why it matters: CPython 3.13 interns a code object's name-like str constants when it makes the code object
(Objects/codeobject.c intern_constants). When a frozenset constant (a set display of 3 or more constants, or a set
literal that `for`, a comprehension or `in` reads) holds such a string and an equal string is already interned as
another object, CPython rebuilds the frozenset once more from its iteration order, which can lay the table out
differently. An equal string is already interned when it is an identifier of the module (the parser interns
every name) or one of the strings interned at startup: this table. dynpy/emit.rs reads it at build time.

The startup strings are the static ones (_Py_ID and _Py_STR in Include/internal/pycore_global_strings.h, fetched
from the pinned tag) and the interpreter's interned dict as the script starts. The dict keeps insertion order, so
a probe script whose first name is a marker reads it with ctypes and keeps the entries before the marker (the
strings interned before the script was compiled). Run with the CPython 3.13.13 the conformance runs compare with:
    python3 tools/gen_interned.py /path/to/python3.13
"""
import os
import re
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src" / "dynpy" / "interned_313.txt"
TAG = "v3.13.13"
HEADER = f"https://raw.githubusercontent.com/python/cpython/{TAG}/Include/internal/pycore_global_strings.h"

PROBE = r'''zqmark0 = 0
import sys, ctypes
maps = []
for l in open('/proc/self/maps'):
    a, perm = l.split()[:2]
    if perm.startswith('r'):
        lo, hi = (int(x, 16) for x in a.split('-'))
        maps.append((lo, hi))
def ok(p):
    return any(lo <= p and p + 64 <= hi for lo, hi in maps)
api = ctypes.pythonapi
api.PyInterpreterState_Get.restype = ctypes.c_void_p
interp = api.PyInterpreterState_Get()
found = None
for off in range(0, 1 << 20, 8):
    a = interp + off
    if not ok(a):
        break
    w = ctypes.c_void_p.from_address(a).value
    if w and w % 8 == 0 and ok(w) and ctypes.c_void_p.from_address(w + 8).value == id(dict):
        d = ctypes.cast(w, ctypes.py_object).value
        api.Py_IncRef(ctypes.py_object(d))  # cast gives a borrowed reference: dropping it would free the dict
        # the first big dict of the interpreter state is cached_objects.interned_strings (checked below)
        if len(d) > 2000:
            found = list(d) if 'zqmark0' in d else None
            break
with open(sys.argv[1], 'w') as f:
    for s in found:
        f.write(s.encode('utf-8', 'surrogatepass').hex() + '\n')
sys.stdout.flush()
'''

NAME = re.compile(r"[A-Za-z0-9_]{2,}")


def main():
    py = sys.argv[1] if len(sys.argv) > 1 else "python3.13"
    version = subprocess.run([py, "-c", "import sys; print(sys.version.split()[0])"], capture_output=True, text=True).stdout.strip()
    if version != TAG[1:]:
        sys.exit(f"{py} is CPython {version}, not {TAG[1:]}")
    header = urllib.request.urlopen(HEADER).read().decode()
    statics = set(re.findall(r"STRUCT_FOR_ID\((\w+)\)", header))
    statics |= set(re.findall(r'STRUCT_FOR_STR\(\w+, "([^"\\]*)"\)', header))
    with tempfile.TemporaryDirectory() as d:
        probe, out = Path(d) / "probe.py", Path(d) / "dict.hex"
        probe.write_text(PROBE)
        # tools/conformance.py run_env and reference(): the same startup as the reference runs
        env = {"PATH": "/usr/bin:/bin", "HOME": d, "TMPDIR": d, "PYTHONUTF8": "1", "PYTHONHASHSEED": "0", "PYTHONDONTWRITEBYTECODE": "1"}
        subprocess.run([py, "-B", "-s", "-P", str(probe), str(out)], cwd=d, env=env)
        keys = [bytes.fromhex(x).decode("utf-8", "surrogatepass") for x in out.read_text().split()]
    if "zqmark0" not in keys:
        sys.exit("the probe did not find the interned dict")
    startup = set(keys[:keys.index("zqmark0")]) | statics
    names = sorted(s for s in startup if NAME.fullmatch(s))
    OUT.write_text("".join(n + "\n" for n in names))
    print(f"{OUT.relative_to(ROOT)}: {len(names)} names ({len(statics)} static strings, {keys.index('zqmark0')} interned at startup)")


if __name__ == "__main__":
    main()
