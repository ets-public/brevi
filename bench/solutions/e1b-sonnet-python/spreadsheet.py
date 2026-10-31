import sys, re
cells = {}
maxc = 0; maxr = 0
def note(c, r):
    global maxc, maxr
    maxc = max(maxc, c); maxr = max(maxr, r)
def ref(s):
    return ord(s[0]) - 64, int(s[1:])
for line in sys.stdin.read().split('\n'):
    if '=' not in line:
        continue
    k, v = line.split('=', 1)
    k = k.strip(); v = v.strip()
    c, r = ref(k)
    note(c, r)
    if v.startswith('='):
        f = v[1:].replace(' ', '').replace('\t', '')
        for m in re.finditer(r'([A-Z])(\d+)', f):
            note(*ref(m.group(0)))
        cells[(c, r)] = ('f', f)
    else:
        cells[(c, r)] = ('n', int(v))
tok_re = re.compile(r'(\d+)|([A-Z]+\d+:[A-Z]+\d+)|([A-Z]+\d+)|([A-Z]+)|(.)')
def tokenize(f):
    return [m.group(0) for m in tok_re.finditer(f)]
def parse(toks):
    pos = [0]
    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None
    def nxt():
        t = peek(); pos[0] += 1; return t
    def expr():
        a = term()
        while peek() in ('+', '-'):
            op = nxt(); b = term(); a = (op, a, b)
        return a
    def term():
        a = unary()
        while peek() in ('*', '/'):
            op = nxt(); b = unary(); a = (op, a, b)
        return a
    def unary():
        if peek() == '-':
            nxt(); return ('neg', unary())
        return atom()
    def atom():
        t = nxt()
        if t == '(':
            e = expr(); nxt(); return e
        if t.isdigit():
            return ('num', int(t))
        if ':' in t:
            a, b = t.split(':')
            return ('range', ref(a), ref(b))
        if t[0].isalpha() and t[-1].isdigit():
            return ('ref', ref(t))
        nxt()  # (
        arg = expr()
        nxt()  # )
        return ('fn', t, arg)
    return expr()
memo = {}; visiting = set()
def getcell(key):
    if key in memo:
        return memo[key]
    if key in visiting:
        return 'C'
    d = cells.get(key)
    if d is None:
        return 0
    if d[0] == 'n':
        return d[1]
    visiting.add(key)
    v = ev(parse(tokenize(d[1])))
    visiting.discard(key)
    memo[key] = v
    return v
def merge(vals):
    if 'C' in vals: return 'C'
    if 'D' in vals: return 'D'
    return None
def ev(n):
    t = n[0]
    if t == 'num': return n[1]
    if t == 'ref': return getcell(n[1])
    if t == 'neg':
        v = ev(n[1]); return v if isinstance(v, str) else -v
    if t in '+-*/':
        a = ev(n[1]); b = ev(n[2])
        e = merge([a, b])
        if e: return e
        if t == '+': return a + b
        if t == '-': return a - b
        if t == '*': return a * b
        if b == 0: return 'D'
        q = abs(a) // abs(b)
        return q if (a < 0) == (b < 0) else -q
    if t == 'fn':
        a = ev(n[2])
        return a
    return None
def evfn(n):
    pass
# redefine ev for fn/range properly
_ev = ev
def ev(n):
    if n[0] == 'fn':
        arg = n[2]
        if arg[0] == 'range':
            (c1, r1), (c2, r2) = arg[1], arg[2]
            vals = [getcell((c, r)) for c in range(min(c1, c2), max(c1, c2) + 1) for r in range(min(r1, r2), max(r1, r2) + 1)]
        else:
            vals = [ev(arg)]
        e = merge(vals)
        if e: return e
        if n[1] == 'SUM': return sum(vals)
        if n[1] == 'MIN': return min(vals)
        return max(vals)
    if n[0] == 'neg':
        v = ev(n[1]); return v if isinstance(v, str) else -v
    if n[0] in '+-*/' and n[0] != 'num':
        a = ev(n[1]); b = ev(n[2])
        e = merge([a, b])
        if e: return e
        t = n[0]
        if t == '+': return a + b
        if t == '-': return a - b
        if t == '*': return a * b
        if b == 0: return 'D'
        q = abs(a) // abs(b)
        return q if (a < 0) == (b < 0) else -q
    return _ev(n)
out = ['\t' + '\t'.join(chr(64 + c) for c in range(1, maxc + 1))]
for r in range(1, maxr + 1):
    row = [str(r)]
    for c in range(1, maxc + 1):
        if (c, r) not in cells:
            row.append('')
        else:
            v = getcell((c, r))
            row.append('#CYCLE' if v == 'C' else '#DIV0' if v == 'D' else str(v))
    out.append('\t'.join(row))
print('\n'.join(out))
