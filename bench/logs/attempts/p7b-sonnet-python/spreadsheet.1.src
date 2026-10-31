import sys, re
cells = {}
maxc = 0; maxr = 0
def note(c, r):
    global maxc, maxr
    maxc = max(maxc, c); maxr = max(maxr, r)
for line in sys.stdin.read().split('\n'):
    if '=' not in line: continue
    k, v = line.split('=', 1)
    k = k.strip(); v = v.strip()
    m = re.fullmatch(r'([A-Z])(\d+)', k)
    if not m: continue
    cells[k] = v
    note(ord(m.group(1)) - 64, int(m.group(2)))
tokre = re.compile(r'\s*(\d+|[A-Z]\d+:[A-Z]\d+|[A-Z]\d+|SUM|MIN|MAX|[-+*/():])')
def tokenize(s):
    s = s.replace(' ', '').replace('\t', '')
    out = []; p = 0
    while p < len(s):
        m = tokre.match(s, p)
        out.append(m.group(1)); p = m.end()
    return out
def parse_all():
    for k, v in cells.items():
        if v.startswith('='):
            for t in tokenize(v[1:]):
                for ref in re.findall(r'[A-Z]\d+', t):
                    note(ord(ref[0]) - 64, int(ref[1:]))
parse_all()
memo = {}
state = {}
def val(ref):
    if ref in memo: return memo[ref]
    if state.get(ref) == 1:
        return (0, {'C'})
    if ref not in cells:
        return (0, set())
    state[ref] = 1
    v = cells[ref]
    if v.startswith('='):
        res = evalf(v[1:])
    else:
        try: res = (int(v), set())
        except ValueError: res = (0, set())
    state[ref] = 2
    memo[ref] = res
    return res
def idiv(a, b):
    q = abs(a) // abs(b)
    return q if (a < 0) == (b < 0) else -q
def evalf(s):
    toks = tokenize(s); pos = [0]; flags = set()
    def peek(): return toks[pos[0]] if pos[0] < len(toks) else None
    def nxt():
        t = peek(); pos[0] += 1; return t
    def expr():
        v = term()
        while peek() in ('+', '-'):
            o = nxt(); b = term()
            v = v + b if o == '+' else v - b
        return v
    def term():
        v = unary()
        while peek() in ('*', '/'):
            o = nxt(); b = unary()
            if o == '*': v = v * b
            elif b == 0: flags.add('D'); v = 0
            else: v = idiv(v, b)
        return v
    def unary():
        if peek() == '-':
            nxt(); return -unary()
        if peek() == '+':
            nxt(); return unary()
        return atom()
    def cellv(r):
        v, f = val(r); flags.update(f); return v
    def rng(t):
        a, b = t.split(':')
        c1, c2 = sorted((ord(a[0]), ord(b[0]))); r1, r2 = sorted((int(a[1:]), int(b[1:])))
        return [cellv(chr(c) + str(r)) for c in range(c1, c2 + 1) for r in range(r1, r2 + 1)]
    def atom():
        t = nxt()
        if t is None: return 0
        if t == '(':
            v = expr()
            if peek() == ')': nxt()
            return v
        if t in ('SUM', 'MIN', 'MAX'):
            nxt()
            xs = rng(nxt())
            if peek() == ')': nxt()
            if not xs: return 0
            return sum(xs) if t == 'SUM' else (min(xs) if t == 'MIN' else max(xs))
        if t.isdigit(): return int(t)
        return cellv(t)
    v = expr()
    return (v, flags)
print('\t' + '\t'.join(chr(64 + c) for c in range(1, maxc + 1)))
for r in range(1, maxr + 1):
    row = [str(r)]
    for c in range(1, maxc + 1):
        k = chr(64 + c) + str(r)
        if k not in cells: row.append(''); continue
        v, f = val(k)
        row.append('#CYCLE' if 'C' in f else '#DIV0' if 'D' in f else str(v))
    print('\t'.join(row))
