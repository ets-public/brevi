import sys, re
sys.setrecursionlimit(100000)
cells = {}
maxc = 0
maxr = 0

def note(c, r):
    global maxc, maxr
    maxc = max(maxc, c)
    maxr = max(maxr, r)

def cref(s):
    return (ord(s[0]) - 64, int(s[1:]))

for line in sys.stdin.read().split('\n'):
    if '=' not in line:
        continue
    k, v = line.split('=', 1)
    k = k.strip().upper()
    v = v.strip()
    if not re.match(r'^[A-Z]\d+$', k):
        continue
    cells[cref(k)] = v
    note(*cref(k))

TOK = re.compile(r'\s*(?:([A-Z]\d+:[A-Z]\d+)|([A-Z]+)\(|([A-Z]\d+)|(\d+)|(.))')

def tokenize(s):
    toks = []
    pos = 0
    s = s.strip()
    while pos < len(s):
        m = TOK.match(s, pos)
        if not m:
            break
        pos = m.end()
        if m.group(1): toks.append(('range', m.group(1)))
        elif m.group(2): toks.append(('func', m.group(2)))
        elif m.group(3): toks.append(('ref', m.group(3)))
        elif m.group(4): toks.append(('num', int(m.group(4))))
        elif m.group(5): toks.append(('op', m.group(5)))
    return toks

def tdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q

def expand(rs):
    a, b = rs.split(':')
    (c1, r1), (c2, r2) = cref(a), cref(b)
    out = []
    for c in range(min(c1, c2), max(c1, c2) + 1):
        for r in range(min(r1, r2), max(r1, r2) + 1):
            out.append((c, r))
    return out

# pre-scan formulas for extents
for key, v in list(cells.items()):
    if v.startswith('='):
        for t in tokenize(v[1:]):
            if t[0] == 'ref':
                note(*cref(t[1]))
            elif t[0] == 'range':
                for cc in expand(t[1]):
                    note(*cc)

memo = {}
state = {}

def get(key):
    if key in memo:
        return memo[key]
    if state.get(key) == 1:
        return (0, True, False)
    if key not in cells:
        return (0, False, False)
    v = cells[key]
    state[key] = 1
    if v.startswith('='):
        res = evalf(v[1:])
    else:
        try:
            res = (int(v), False, False)
        except ValueError:
            res = (0, False, False)
    state[key] = 2
    memo[key] = res
    return res

def evalf(s):
    toks = tokenize(s)
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else (None, None)

    def nxt():
        t = peek()
        pos[0] += 1
        return t

    def expr():
        v, cy, dz = term()
        while peek() in (('op', '+'), ('op', '-')):
            op = nxt()[1]
            v2, c2, d2 = term()
            v = v + v2 if op == '+' else v - v2
            cy |= c2; dz |= d2
        return (v, cy, dz)

    def term():
        v, cy, dz = unary()
        while peek() in (('op', '*'), ('op', '/')):
            op = nxt()[1]
            v2, c2, d2 = unary()
            cy |= c2; dz |= d2
            if op == '*':
                v = v * v2
            elif v2 == 0:
                dz = True; v = 0
            else:
                v = tdiv(v, v2)
        return (v, cy, dz)

    def unary():
        if peek() == ('op', '-'):
            nxt()
            v, cy, dz = unary()
            return (-v, cy, dz)
        if peek() == ('op', '+'):
            nxt()
            return unary()
        return atom()

    def atom():
        t = nxt()
        if t[0] == 'num':
            return (t[1], False, False)
        if t[0] == 'ref':
            return get(cref(t[1]))
        if t[0] == 'op' and t[1] == '(':
            r = expr()
            nxt()
            return r
        if t[0] == 'func':
            if peek() and peek()[0] == 'range':
                rs = nxt()[1]
                cl = expand(rs)
            else:
                # single arg expression fallback
                r = expr()
                nxt()
                return r
            nxt()  # ')'
            vals = [get(c) for c in cl]
            cy = any(x[1] for x in vals); dz = any(x[2] for x in vals)
            nums = [x[0] for x in vals]
            f = {'SUM': sum, 'MIN': min, 'MAX': max}[t[1]]
            return (f(nums) if nums else 0, cy, dz)
        return (0, False, False)

    return expr()

if cells or maxc:
    print("\t" + "\t".join(chr(64 + c) for c in range(1, maxc + 1)))
    for r in range(1, maxr + 1):
        row = [str(r)]
        for c in range(1, maxc + 1):
            if (c, r) not in cells:
                row.append("")
                continue
            v, cy, dz = get((c, r))
            row.append("#CYCLE" if cy else "#DIV0" if dz else str(v))
        print("\t".join(row))
