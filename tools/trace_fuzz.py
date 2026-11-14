#!/usr/bin/env python3
"""Random programs in the statement set sys.settrace supports (src/dynpy/trace_check.rs), traced by CPython and by a
dynamic build, the event logs compared exactly.

  python3 tools/trace_fuzz.py --python PY [--seeds 1-20] [--n 10] [--keep DIR]

Each program defines N random functions (and classes) from the checked subset: assignments, augmented assignments,
expression statements, pass, return, raise, assert, if/elif/else, match/case, try/finally, nested defs, classes with
plain and @staticmethod methods, decorated defs, calls between them. The module then sets a trace function and calls
each one. The trace function prints every event with co_name, f_lineno - co_firstlineno and, for "exception", the
exception's type; it varies by seed: a global function that returns itself, one that returns a separate local
function, one that returns None for some frames, one that raises at a chosen event (turning tracing off), one that
calls settrace(None) part way. Every program must build (a refusal is a FAIL: the generator only writes the
subset), and its output must be CPython's byte for byte. Exit 0 when every seed matches. Programs are built with
BREVI_DYN=1 into a temp folder, never into the tree.
"""
import argparse, os, random, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

TRACERS = {
    'self': '''def tr(frame, event, arg):
    if event == "exception":
        print("T", event, frame.f_code.co_name, frame.f_lineno - frame.f_code.co_firstlineno, arg[0].__name__)
    else:
        print("T", event, frame.f_code.co_name, frame.f_lineno - frame.f_code.co_firstlineno)
    return tr
''',
    'local': '''def lt(frame, event, arg):
    print("L", event, frame.f_code.co_name, frame.f_lineno - frame.f_code.co_firstlineno)
    return lt
def tr(frame, event, arg):
    print("T", event, frame.f_code.co_name, frame.f_lineno - frame.f_code.co_firstlineno)
    return lt
''',
    'none': '''def lt(frame, event, arg):
    print("L", event, frame.f_code.co_name, frame.f_lineno - frame.f_code.co_firstlineno)
    if event == "line":
        return None
    return lt
def tr(frame, event, arg):
    print("T", event, frame.f_code.co_name, frame.f_lineno - frame.f_code.co_firstlineno)
    if frame.f_code.co_name in SKIP:
        return None
    return lt
''',
    'raise': '''class Stop(Exception):
    pass
def tr(frame, event, arg):
    C[0] += 1
    print("T", event, frame.f_code.co_name, frame.f_lineno - frame.f_code.co_firstlineno, C[0])
    if C[0] == LIMIT:
        raise Stop(C[0])
    return tr
''',
    'off': '''def tr(frame, event, arg):
    C[0] += 1
    print("T", event, frame.f_code.co_name, frame.f_lineno - frame.f_code.co_firstlineno)
    if C[0] == LIMIT:
        sys.settrace(None)
    return tr
''',
}

EXC = ['ValueError', 'KeyError', 'TypeError', 'IndexError']


class Gen:
    def __init__(self, r):
        self.r = r
        self.n = 0
        self.funcs = []  # (name, nargs) callable from later functions

    def name(self, p):
        self.n += 1
        return '%s%d' % (p, self.n)

    def expr(self, vs):
        r = self.r
        k = r.randint(0, 9)
        if k <= 2 or not vs:
            return str(r.randint(-3, 9))
        v = r.choice(vs)
        if k == 3:
            return '%s + %d' % (v, r.randint(1, 4))
        if k == 4:
            return '%s * 2' % v
        if k == 5:
            return '%s - %s' % (v, r.choice(vs))
        if k == 6 and self.funcs:
            f, na = r.choice(self.funcs)
            return '%s(%s)' % (f, ', '.join(r.choice(vs) for _ in range(na)))
        if k == 7:
            return '[%s, %d][0]' % (v, r.randint(0, 3))
        return v

    def cond(self, vs):
        r = self.r
        k = r.randint(0, 6)
        if k == 0:
            return r.choice(['True', 'False', '0', '1'])
        if not vs:
            return 'True'
        return '%s %s %d' % (r.choice(vs), r.choice(['>', '<', '==', '!=', '>=']), r.randint(-1, 6))

    def block(self, ind, vs, depth, fn):
        r = self.r
        out = []
        for _ in range(r.randint(1, 4)):
            out += self.stmt(ind, vs, depth, fn)
            if out and out[-1].strip().split(' ')[0] in ('return', 'raise'):
                break
        if len(out) == 1 and out[0].strip().startswith('"'):
            out[0] = out[0].replace('"s"', 'pass')  # a def whose body is only its docstring is refused (by design)
        return out

    def stmt(self, ind, vs, depth, fn):
        r = self.r
        p = '    ' * ind
        k = r.randint(0, 19 if depth < 3 else 9)
        if k <= 2:
            v = r.choice(['a', 'b', 'c', 'd'])
            line = '%s%s = %s' % (p, v, self.expr(vs))
            if v not in vs:
                vs.append(v)
            return [line]
        if k == 3 and vs:
            return ['%s%s %s= %d' % (p, r.choice(vs), r.choice('+-*'), r.randint(1, 3))]
        if k == 4:
            return [p + r.choice(['pass', '1', 'None', '"s"', self.expr(vs)])]
        if k == 5:
            return ['%sreturn %s' % (p, self.expr(vs))]
        if k == 6:
            return ['%sraise %s(%s)' % (p, r.choice(EXC), self.expr(vs))]
        if k == 7:
            return ['%sassert %s' % (p, self.cond(vs))]
        if k == 8 and self.funcs:
            f, na = r.choice(self.funcs)
            return ['%s%s(%s)' % (p, f, ', '.join(self.expr(vs) for _ in range(na)))]
        if k in (9, 10, 11):
            out = ['%sif %s:' % (p, self.cond(vs))] + self.block(ind + 1, list(vs), depth + 1, fn)
            for _ in range(r.randint(0, 2)):
                out += ['%selif %s:' % (p, self.cond(vs))] + self.block(ind + 1, list(vs), depth + 1, fn)
            if r.random() < 0.5:
                out += ['%selse:' % p] + self.block(ind + 1, list(vs), depth + 1, fn)
            return out
        if k in (12, 13) and vs:
            out = ['%smatch %s:' % (p, r.choice(vs))]
            for _ in range(r.randint(1, 3)):
                out += ['%s    case %s:' % (p, r.choice(['0', '1', '2', '-1', '3 | 4', '5 if %s' % self.cond(vs)]))]
                out += self.block(ind + 2, list(vs), depth + 1, fn)
            if r.random() < 0.5:
                out += ['%s    case _:' % p] + self.block(ind + 2, list(vs), depth + 1, fn)
            return out
        if k in (14, 15):
            out = ['%stry:' % p] + self.block(ind + 1, vs, depth + 1, fn)
            out += ['%sfinally:' % p] + self.block(ind + 1, list(vs), depth + 1, fn)
            return out
        if k == 16:
            g = self.name('inner')
            out = ['%sdef %s(x):' % (p, g)] + self.block(ind + 1, ['x'], depth + 1, g)
            out += ['%s%s = %s(%s)' % (p, 'a', g, vs[0] if vs else '1')]
            if 'a' not in vs:
                vs.append('a')
            return out
        if k == 17:
            c = self.name('K')
            out = ['%sclass %s:' % (p, c), '%s    z = %d' % (p, r.randint(0, 5))]
            out += ['%s    def __init__(self, x):' % p, '%s        self.x = x' % p]
            out += ['%s    def m(self):' % p, '%s        return self.x + self.z' % p]
            if r.random() < 0.5:
                out += ['%s    @staticmethod' % p, '%s    def s(y):' % p] + self.block(ind + 2, ['y'], depth + 2, 's')
            out += ['%sb = %s(%s).m()' % (p, c, vs[0] if vs else '2')]
            if 'b' not in vs:
                vs.append('b')
            return out
        if k == 18:
            g = self.name('dec')
            out = ['%sdef %s(h):' % (p, g), '%s    return h' % p]
            out += ['%s@%s' % (p, g), '%sdef %s_f(x):' % (p, g)] + self.block(ind + 1, ['x'], depth + 1, g)
            out += ['%sa = %s_f(%s)' % (p, g, vs[0] if vs else '0')]
            if 'a' not in vs:
                vs.append('a')
            return out
        return ['%s%s = %d' % (p, r.choice(['a', 'b']), r.randint(0, 9))] if not vs else ['%s%s += 1' % (p, r.choice(vs))]

    def func(self):
        f = self.name('f')
        na = self.r.randint(0, 2)
        args = ['p%d' % i for i in range(na)]
        body = self.block(1, list(args), 0, f)
        self.funcs.append((f, na))
        return f, na, ['def %s(%s):' % (f, ', '.join(args))] + body


def program(seed, n):
    r = random.Random(seed)
    g = Gen(r)
    kind = list(TRACERS)[seed % len(TRACERS)]
    src = ['import sys', 'C = [0]', 'LIMIT = %d' % r.randint(3, 60)]
    defs = [g.func() for _ in range(n)]
    for _, _, lines in defs:
        src += lines
    src.append('SKIP = [%s]' % ', '.join('"%s"' % f for f, _, _ in defs if r.random() < 0.3))
    src += TRACERS[kind].rstrip('\n').split('\n')
    src.append('sys.settrace(tr)')
    for f, na, _ in defs:
        src += ['try:', '    print("R", %s(%s))' % (f, ', '.join(str(r.randint(-1, 5)) for _ in range(na))),
                'except Exception as e:', '    print("E", type(e).__name__)']
    src.append('sys.settrace(None)')
    src.append('print("end", sys.gettrace())')
    return kind, '\n'.join(src) + '\n'


def seeds(s):
    a, _, b = s.partition('-')
    return range(int(a), int(b or a) + 1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--python', default=sys.executable)
    ap.add_argument('--seeds', default='1-20')
    ap.add_argument('--n', type=int, default=10)
    ap.add_argument('--keep')
    a = ap.parse_args()
    brevi = ROOT / 'target' / 'release' / 'brevi'
    bad = 0
    with tempfile.TemporaryDirectory() as d:
        for seed in seeds(a.seeds):
            kind, src = program(seed, a.n)
            p = Path(a.keep or d) / ('t%d.py' % seed)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(src)
            py = subprocess.run([a.python, str(p)], capture_output=True, text=True, timeout=120)
            want = py.stdout + py.stderr
            if py.returncode:
                print('seed %d: CPython failed (generator bug)\n%s' % (seed, want[-800:]))
                bad += 1
                continue
            exe = Path(d) / ('t%d.bin' % seed)
            b = subprocess.run([str(brevi), 'build', str(p), '-o', str(exe)], capture_output=True, text=True,
                               timeout=900, env=dict(os.environ, BREVI_DYN='1'))
            if b.returncode:
                print('seed %d (%s): FAIL build refused\n%s' % (seed, kind, (b.stdout + b.stderr)[-800:]))
                bad += 1
                continue
            g = subprocess.run([str(exe)], capture_output=True, text=True, timeout=120)
            got = g.stdout + g.stderr
            if got == want:
                print('seed %d (%s): ok, %d events' % (seed, kind, want.count('\n')))
                continue
            bad += 1
            wl, gl = want.split('\n'), got.split('\n')
            i = next((i for i in range(max(len(wl), len(gl))) if (wl[i:i + 1] or [None]) != (gl[i:i + 1] or [None])), 0)
            print('seed %d (%s): FAIL at output line %d of %s\n  want %r\n  got  %r' % (
                seed, kind, i + 1, p, wl[i] if i < len(wl) else None, gl[i] if i < len(gl) else None))
    print('%d of %d seeds differ' % (bad, len(seeds(a.seeds))))
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
