#!/usr/bin/env python3
"""Random bytes (and bytearray) method calls, run by CPython and by a dynamic build, compared line for line.

  python3 tools/bytes_fuzz.py --python PY [--seeds 1-8] [--n 400] [--type bytes|bytearray|buffer] [--keep DIR]

Each case calls one method (or operator) on a random receiver with random arguments, mostly valid and some of the
wrong type or out of range, and prints the repr of the result, or the exception's type and message; for bytearray
it also prints the receiver afterwards, so mutation and aliasing are compared too. Exit 0 when every seed's output
is the same as CPython's. Generated programs are built with BREVI_DYN=1 into a temp folder (never into the tree).
"""
import argparse, os, random, re, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

ALPHA = [b'a', b'b', b'A', b'B', b' ', b'\t', b'\n', b'\r', b'x', b'X', b'1', b'0', b'.', b'-', b'_', b'\xe9', b'\xff', b'\x00', b'\x0b', b'\x0c', b'Ab', b'aB', b'ab', b'ba']
METHODS = """capitalize center count decode endswith expandtabs find hex index isalnum isalpha isascii isdigit islower
isspace istitle isupper join ljust lower lstrip partition removeprefix removesuffix replace rfind rindex rjust rpartition
rsplit rstrip split splitlines startswith strip swapcase title translate upper zfill""".split()
MUTATORS = "append extend insert pop remove clear reverse copy setitem setslice delitem delslice iadd imul".split()


def rbytes(r, k=None):
    k = r.randint(0, 7) if k is None else k
    return b''.join(r.choice(ALPHA) for _ in range(k))


def lit(r, b, t):
    """the receiver or a bytes-like argument as source text"""
    if t == 'bytearray' and r.random() < 0.5:
        return 'bytearray(%r)' % b
    return repr(b)


def rint(r):
    return r.choice([-100, -9, -3, -2, -1, 0, 1, 2, 3, 4, 5, 7, 10, 100, 'True', 'None', '2**70', '-2**70'])


def bad(r):
    return r.choice(["'a'", '1.5', '[1]', '()', 'None', '"ab"', '[97, 98]', '300', '-1'])


def args_for(r, m, t):
    B = lambda: lit(r, rbytes(r, r.randint(0, 3)), t)  # noqa: E731
    pick = r.random()
    if pick < 0.08:
        return [bad(r)]
    if m in ('center', 'ljust', 'rjust'):
        a = [str(r.randint(-2, 12))]
        if r.random() < 0.6:
            a.append(r.choice([repr(b'*'), repr(b'ab'), lit(r, b'-', t), repr(b'')]))
        return a
    if m == 'zfill':
        return [str(r.randint(-2, 12))]
    if m == 'expandtabs':
        return [] if r.random() < 0.3 else [r.choice(['0', '1', '2', '4', '8', '-1', 'tabsize=3'])]
    if m in ('count', 'find', 'index', 'rfind', 'rindex'):
        a = [B() if r.random() < 0.8 else str(r.randint(-2, 260))]
        for _ in range(r.randint(0, 2)):
            a.append(str(rint(r)))
        return a
    if m in ('startswith', 'endswith'):
        a = [B() if r.random() < 0.7 else '(%s, %s)' % (B(), B())]
        for _ in range(r.randint(0, 2)):
            a.append(str(rint(r)))
        return a
    if m in ('partition', 'rpartition', 'removeprefix', 'removesuffix'):
        return [B()]
    if m == 'replace':
        a = [B(), B()]
        if r.random() < 0.4:
            a.append(str(rint(r)) if r.random() < 0.7 else 'count=%s' % rint(r))
        return a
    if m in ('split', 'rsplit'):
        a = []
        if r.random() < 0.7:
            a.append(r.choice(['None', B(), B(), 'sep=' + B()]))
        if r.random() < 0.5:
            kw = any('=' in x for x in a)
            a.append('maxsplit=%s' % rint(r) if kw or r.random() < 0.5 else str(rint(r)))
        return a
    if m in ('strip', 'lstrip', 'rstrip'):
        return [] if r.random() < 0.4 else [r.choice(['None', B(), B()])]
    if m == 'splitlines':
        return [] if r.random() < 0.5 else [r.choice(['True', 'False', 'keepends=True', '1'])]
    if m == 'join':
        return ['[%s]' % ', '.join(B() for _ in range(r.randint(0, 3)))]
    if m == 'hex':
        a = []
        if r.random() < 0.6:
            a.append(r.choice(["':'", "b'-'", "' '", "''", "'ab'", "'\\xe9'"]))
            if r.random() < 0.6:
                a.append(str(r.choice([-3, -2, -1, 0, 1, 2, 3])))
        return a
    if m == 'decode':
        a = []
        if r.random() < 0.7:
            a.append(r.choice(["'utf-8'", "'ascii'", "'latin-1'", "encoding='utf8'"]))
            if r.random() < 0.5:
                a.append(r.choice(["'strict'", "'ignore'", "'replace'", "errors='backslashreplace'"]))
        return a
    if m == 'translate':
        tab = r.choice(['None', "bytes.maketrans(b'ab', b'xy')", 'bytes(range(256))[::-1]', "b'x'"])
        a = [tab]
        if r.random() < 0.5:
            a.append(r.choice([B(), 'delete=' + B()]))
        return a
    return []


def fix_order(args):
    """no positional argument after a keyword one (a SyntaxError)"""
    out, kw = [], False
    for x in args:
        k = re.match(r'[a-z_]+=', x) is not None
        if kw and not k:
            continue
        kw |= k
        out.append(x)
    return out


def case_bytes(r, i, t):
    recv = rbytes(r)
    m = r.choice(METHODS)
    expr = '%s.%s(%s)' % (lit(r, recv, t), m, ', '.join(fix_order(args_for(r, m, t))))
    return 'try:\n    print(%d, repr(%s))\nexcept Exception as e:\n    print(%d, type(e).__name__, e)\n' % (i, expr, i)


def case_ops(r, i, t):
    a, b = lit(r, rbytes(r), t), lit(r, rbytes(r), t)
    e = r.choice(['%s + %s' % (a, b), '%s * %s' % (a, rint(r)), '%s == %s' % (a, b), '%s < %s' % (a, b),
                  '%s >= %s' % (a, b), '%s in %s' % (b, a), '%s in %s' % (r.randint(-1, 256), a),
                  '%s[%s]' % (a, rint(r)), '%s[%s:%s:%s]' % (a, rint(r), rint(r), r.choice(['1', '-1', '2', '-3', 'None', '0'])),
                  'list(%s)' % a, 'len(%s)' % a, 'bool(%s)' % a, 'bytes(%s)' % a, '%s(%s)' % (t, a),
                  'hash(%s) == hash(%s)' % (a, a), '%s == %r' % (a, 'x'), 'sorted([%s, %s])' % (a, b),
                  'bytes.fromhex(%r)' % r.choice(['', 'ab', 'a b', '0g', ' 01 02 ', 'abc']), "bytes.maketrans(%s, %s)[:4]" % (a, a)])
    return 'try:\n    print(%d, repr(%s))\nexcept Exception as e:\n    print(%d, type(e).__name__, e)\n' % (i, e, i)


def case_mut(r, i):
    recv = rbytes(r)
    m = r.choice(MUTATORS)
    B = lambda: repr(rbytes(r, r.randint(0, 3)))  # noqa: E731
    stmt = {
        'append': lambda: 'x.append(%s)' % r.choice([str(r.randint(-1, 256)), "b'a'", '97', 'True']),
        'extend': lambda: 'x.extend(%s)' % r.choice([B(), '[1, 2]', 'bytearray(b"z")', 'x', 'range(3)', '[300]', "'ab'"]),
        'insert': lambda: 'x.insert(%s, %s)' % (rint(r), r.choice(['65', '256', "b'a'"])),
        'pop': lambda: 'print(x.pop(%s))' % r.choice(['', '0', '-1', '5', '-9']),
        'remove': lambda: 'x.remove(%s)' % r.choice(['97', '120', '256', "b'a'"]),
        'clear': lambda: 'x.clear()',
        'reverse': lambda: 'x.reverse()',
        'copy': lambda: 'y = x.copy(); y.append(1); print(y is x, y)',
        'setitem': lambda: 'x[%s] = %s' % (rint(r), r.choice(['0', '255', '256', '-1', "b'a'", 'True'])),
        'setslice': lambda: 'x[%s:%s%s] = %s' % (rint(r), rint(r), r.choice(['', ':-1', ':2', ':1']), r.choice([B(), 'x', '[1, 2, 3]', 'b""', 'memoryview(b"ab")', 'bytearray(b"qq")'])),
        'delitem': lambda: 'del x[%s]' % rint(r),
        'delslice': lambda: 'del x[%s:%s%s]' % (rint(r), rint(r), r.choice(['', ':2', ':-1', ':-2'])),
        'iadd': lambda: 'x += %s' % r.choice([B(), 'x', '[1]', "'a'"]),
        'imul': lambda: 'x *= %s' % rint(r),
    }[m]()
    return ('try:\n    x = bytearray(%r); z = x\n    %s\n    print(%d, z is x, x)\nexcept Exception as e:\n'
            '    print(%d, type(e).__name__, e, x)\n') % (recv, stmt, i, i)


def case_mv(r, i):
    """memoryview over a bytearray (or bytes): reads, writes through the view, resizes while exported, release"""
    recv = rbytes(r, r.randint(0, 6))
    base = r.choice(['bytearray(%r)' % recv, repr(recv)])
    I = lambda: str(rint(r))  # noqa: E731
    St = lambda: r.choice(['', ':2', ':-1', ':0'])  # noqa: E731
    val = lambda: r.choice(['0', '255', '256', '-1', "b'a'", 'True', '1.0'])  # noqa: E731
    src = lambda: r.choice(["b'ab'", "b''", 'bytearray(b"q")', '[1]', '"s"', 'b"abcd"', 'memoryview(b"z")'])  # noqa: E731
    ops = [lambda: 'print(m[%s])' % I(), lambda: 'm[%s] = %s' % (I(), val()), lambda: 'm[%s:%s%s] = %s' % (I(), I(), St(), src()),
           lambda: 'm = m[%s:%s%s]; print(m.tobytes())' % (I(), I(), St()), lambda: 'x.append(1)', lambda: "x.extend(b'zz')",
           lambda: 'del x[0]', lambda: "x[0:1] = b''", lambda: 'x[0] = 7', lambda: "x += b'q'", lambda: 'x.pop()',
           lambda: 'x.clear()', lambda: 'm.release()', lambda: 'print(m.tolist())', lambda: 'print(m.hex())',
           lambda: 'print(len(m), m.nbytes, m.readonly, m.shape, m.strides, m.contiguous)',
           lambda: 'print(m == x, m == bytes(x))', lambda: "print(hash(m) == hash(bytes(m)), hash(m[:0]))", lambda: 'print(bytes(m))',
           lambda: 'print(list(m))', lambda: 'r = m.toreadonly(); print(r.readonly); r[0] = 1',
           lambda: 'with memoryview(x) as m2: m2[0] = 66', lambda: 'x.insert(0, 1)', lambda: 'x *= 2', lambda: 'x.reverse()',
           lambda: 'print(m[%s:%s%s].tolist())' % (I(), I(), St()),
           lambda: 'print(bytearray(m), x.extend(m), x)', lambda: 'y = bytearray(b"a"); y += m; print(y)',
           lambda: "import re; print(re.findall(rb'.', m))"]
    body = ''.join('    try:\n        %s\n    except Exception as e:\n        print(%d, type(e).__name__, e)\n' % (r.choice(ops)(), i)
                   for _ in range(r.randint(1, 4)))
    return ('try:\n    x = %s; m = memoryview(x)\n%s    print(%d, bytes(x))\n'
            'except Exception as e:\n    print(%d, type(e).__name__, e)\n') % (base, body, i, i)


def program(seed, n, t):
    r = random.Random(seed)
    out = []
    for i in range(n):
        k = r.random()
        if t == 'buffer':
            out.append(case_mut(r, i) if k < 0.5 else case_mv(r, i))
        elif t == 'bytearray' and k < 0.25:
            out.append(case_mut(r, i))
        elif t == 'bytearray' and k < 0.4:
            out.append(case_mv(r, i))
        elif k < 0.8:
            out.append(case_bytes(r, i, t))
        else:
            out.append(case_ops(r, i, t))
    return ''.join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--python', required=True)
    ap.add_argument('--seeds', default='1-8')
    ap.add_argument('--n', type=int, default=400)
    ap.add_argument('--type', default='bytes', choices=['bytes', 'bytearray', 'buffer'])
    ap.add_argument('--keep', help='write each seed\'s program and outputs here')
    a = ap.parse_args()
    lo, _, hi = a.seeds.partition('-')
    brevi = ROOT / 'target' / 'release' / 'brevi'
    bad_seeds = 0
    with tempfile.TemporaryDirectory() as d:
        for seed in range(int(lo), int(hi or lo) + 1):
            src = program(seed, a.n, a.type)
            p = Path(d) / ('f%d.py' % seed)
            p.write_text(src)
            py = subprocess.run([a.python, str(p)], capture_output=True, text=True, timeout=120)
            if py.returncode:
                sys.exit('the generated program fails in CPython (a fuzzer bug):\n' + py.stderr[-800:])
            want = py.stdout
            b = subprocess.run([str(brevi), 'build', str(p), '-o', str(Path(d) / ('f%d.bin' % seed))], capture_output=True,
                               text=True, timeout=900, env=dict(os.environ, BREVI_DYN='1'))
            got = b.stdout + b.stderr if b.returncode else subprocess.run([str(Path(d) / ('f%d.bin' % seed))], capture_output=True,
                                                                          text=True, timeout=120).stdout
            if a.keep:
                k = Path(a.keep)
                k.mkdir(parents=True, exist_ok=True)
                (k / p.name).write_text(src)
                (k / ('f%d.py.txt' % seed)).write_text(want)
                (k / ('f%d.br.txt' % seed)).write_text(got)
            if got != want:
                bad_seeds += 1
                w, g = want.splitlines(), got.splitlines()
                diffs = [(x, y) for x, y in zip(w, g) if x != y]
                print('seed %d: %d lines differ (of %d)%s' % (seed, len(diffs) + abs(len(w) - len(g)), len(w),
                                                              ' BUILD FAILED' if b.returncode else ''))
                for x, y in diffs[:8]:
                    print('  python:', x)
                    print('  brevi: ', y)
                if b.returncode:
                    print('  ' + got[-600:])
            else:
                print('seed %d: same (%d lines)' % (seed, len(want.splitlines())))
    print('FAIL' if bad_seeds else 'PASS')
    return 1 if bad_seeds else 0


if __name__ == '__main__':
    sys.exit(main())
