import sys, re

cells = {}
maxc = 0
maxr = 0

def note(col, row):
    global maxc, maxr
    maxc = max(maxc, col); maxr = max(maxr, row)

TOK = re.compile(r'\s*(\d+|[A-Z]+\d*|[-+*/():])')
CYC, DIV = 1, 2

def tokenize(s):
    s = s.replace(' ', '').replace('\t', '')
    return re.findall(r'\d+|[A-Z]+\d+|[A-Z]+|[-+*/():()]', s)

def refpos(t):
    m = re.fullmatch(r'([A-Z])(\d+)', t)
    return ord(m.group(1)) - 64, int(m.group(2))

memo = {}
active = set()

def cellval(col, row):
    key = (col, row)
    if key in memo:
        return memo[key]
    if key in active:
        return (0, CYC)
    c = cells.get(key)
    if c is None:
        return (0, 0)
    active.add(key)
    if isinstance(c, int):
        r = (c, 0)
    else:
        r = evalf(c)
    active.discard(key)
    memo[key] = r
    return r

def evalf(toks):
    pos = [0]
    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None
    def nxt():
        t = peek(); pos[0] += 1; return t
    def expr():
        v, f = term()
        while peek() in ('+', '-'):
            op = nxt()
            v2, f2 = term()
            f |= f2
            v = v + v2 if op == '+' else v - v2
        return v, f
    def term():
        v, f = unary()
        while peek() in ('*', '/'):
            op = nxt()
            v2, f2 = unary()
            f |= f2
            if op == '*':
                v = v * v2
            else:
                if v2 == 0:
                    f |= DIV; v = 0
                else:
                    q = abs(v) // abs(v2)
                    v = q if (v >= 0) == (v2 > 0) else -q
        return v, f
    def unary():
        if peek() == '-':
            nxt()
            v, f = unary()
            return -v, f
        if peek() == '+':
            nxt()
            return unary()
        return atom()
    def atom():
        t = nxt()
        if t == '(':
            v, f = expr()
            nxt()
            return v, f
        if t is None:
            return 0, 0
        if t.isdigit():
            return int(t), 0
        if t in ('SUM', 'MIN', 'MAX'):
            nxt()  # (
            a = nxt()
            c1, r1 = refpos(a)
            if peek() == ':':
                nxt()
                c2, r2 = refpos(nxt())
            else:
                c2, r2 = c1, r1
            nxt()  # )
            vals = []; f = 0
            for cc in range(min(c1, c2), max(c1, c2) + 1):
                for rr in range(min(r1, r2), max(r1, r2) + 1):
                    v, ff = cellval(cc, rr)
                    vals.append(v); f |= ff
            if t == 'SUM':
                return sum(vals), f
            if t == 'MIN':
                return min(vals), f
            return max(vals), f
        return cellval(*refpos(t))
    return expr()

def scan(toks):
    i = 0
    while i < len(toks):
        t = toks[i]
        if re.fullmatch(r'[A-Z]\d+', t):
            note(*refpos(t))
        i += 1

for line in sys.stdin:
    line = line.strip()
    if not line or '=' not in line:
        continue
    name, content = line.split('=', 1)
    name = name.strip(); content = content.strip()
    col, row = refpos(name)
    note(col, row)
    if content.startswith('='):
        toks = tokenize(content[1:])
        scan(toks)
        cells[(col, row)] = toks
    else:
        cells[(col, row)] = int(content)

print('\t' + '\t'.join(chr(64 + c) for c in range(1, maxc + 1)))
for r in range(1, maxr + 1):
    row = [str(r)]
    for c in range(1, maxc + 1):
        if (c, r) not in cells:
            row.append('')
            continue
        v, f = cellval(c, r)
        row.append('#CYCLE' if f & CYC else '#DIV0' if f & DIV else str(v))
    print('\t'.join(row))
