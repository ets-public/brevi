import sys, re
cells = {}
maxc = 0; maxr = 0
def note(c, r):
    global maxc, maxr
    maxc = max(maxc, c); maxr = max(maxr, r)
for line in sys.stdin.read().split('\n'):
    if '=' not in line:
        continue
    k, v = line.split('=', 1)
    k = k.strip(); v = v.strip()
    m = re.fullmatch(r'([A-Z])(\d+)', k)
    if not m:
        continue
    note(ord(m.group(1)) - 64, int(m.group(2)))
    if v.startswith('='):
        f = v[1:].replace(' ', '')
        for mm in re.finditer(r'([A-Z])(\d+)', f):
            note(ord(mm.group(1)) - 64, int(mm.group(2)))
        cells[k] = f
    else:
        cells[k] = int(v)
memo = {}
active = set()
def comb(a, b):
    return a if isinstance(a, str) and a == 'C' else (b if isinstance(b, str) and b == 'C' else (a if isinstance(a, str) else (b if isinstance(b, str) else None)))
def val(k):
    if k in memo:
        return memo[k]
    if k in active:
        return 'C'
    c = cells.get(k)
    if c is None:
        return 0
    if isinstance(c, int):
        return c
    active.add(k)
    r = evalf(c)
    active.discard(k)
    memo[k] = r
    return r
def evalf(f):
    toks = re.findall(r'[A-Z]+\d+|[A-Z]+|\d+|.', f)
    pos = [0]
    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None
    def nxt():
        t = peek(); pos[0] += 1; return t
    def bin_(op, a, b):
        e = comb(a, b)
        if e: return e
        if op == '+': return a + b
        if op == '-': return a - b
        if op == '*': return a * b
        if b == 0: return 'D'
        q = abs(a) // abs(b)
        return q if (a < 0) == (b < 0) else -q
    def expr():
        a = term()
        while peek() in ('+', '-'):
            op = nxt(); b = term(); a = bin_(op, a, b)
        return a
    def term():
        a = unary()
        while peek() in ('*', '/'):
            op = nxt(); b = unary(); a = bin_(op, a, b)
        return a
    def unary():
        if peek() == '-':
            nxt(); a = unary()
            return a if isinstance(a, str) else -a
        if peek() == '+':
            nxt(); return unary()
        return atom()
    def atom():
        t = nxt()
        if t == '(':
            a = expr(); nxt(); return a
        if t.isdigit():
            return int(t)
        if t in ('SUM', 'MIN', 'MAX'):
            nxt()
            c1 = nxt(); nxt(); c2 = nxt(); nxt()
            m1 = re.fullmatch(r'([A-Z])(\d+)', c1); m2 = re.fullmatch(r'([A-Z])(\d+)', c2)
            ca, cb = sorted((ord(m1.group(1)), ord(m2.group(1))))
            ra, rb = sorted((int(m1.group(2)), int(m2.group(2))))
            vs = [val('%s%d' % (chr(c), r)) for c in range(ca, cb + 1) for r in range(ra, rb + 1)]
            err = None
            for v in vs:
                if isinstance(v, str):
                    err = 'C' if (v == 'C' or err == 'C') else 'D'
            if err: return err
            return sum(vs) if t == 'SUM' else (min(vs) if t == 'MIN' else max(vs))
        return val(t)
    return expr()
print('\t' + '\t'.join(chr(64 + c) for c in range(1, maxc + 1)))
for r in range(1, maxr + 1):
    row = [str(r)]
    for c in range(1, maxc + 1):
        k = '%s%d' % (chr(64 + c), r)
        if k not in cells:
            row.append('')
        else:
            v = val(k)
            row.append('#CYCLE' if v == 'C' else '#DIV0' if v == 'D' else str(v))
    print('\t'.join(row))
