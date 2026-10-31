import sys, re

cells = {}
maxc = 0
maxr = 0

def note(col, row):
    global maxc, maxr
    maxc = max(maxc, col)
    maxr = max(maxr, row)

refre = re.compile(r'([A-Z])(\d{1,2})')
for line in sys.stdin.read().split("\n"):
    if "=" not in line:
        continue
    left, right = line.split("=", 1)
    name = left.strip()
    content = right.strip()
    m = re.fullmatch(r'([A-Z])(\d+)', name)
    if not m:
        continue
    col = ord(m.group(1)) - 64
    row = int(m.group(2))
    cells[(col, row)] = content
    note(col, row)
    if content.startswith("="):
        for r in refre.finditer(content[1:]):
            note(ord(r.group(1)) - 64, int(r.group(2)))

tokre = re.compile(r'\s*(?:(\d+)|([A-Z]+\d*)|(.))')

def tokenize(s):
    toks = []
    pos = 0
    s = s.strip()
    while pos < len(s):
        m = tokre.match(s, pos)
        if not m:
            break
        if m.group(1) is not None:
            toks.append(("n", int(m.group(1))))
        elif m.group(2) is not None:
            toks.append(("id", m.group(2)))
        else:
            toks.append(("op", m.group(3)))
        pos = m.end()
    return toks

memo = {}
inprog = set()
CYC = 1
DIV = 2

def tdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q

def cellval(col, row):
    k = (col, row)
    if k in memo:
        return memo[k]
    if k in inprog:
        return (0, CYC)
    c = cells.get(k)
    if c is None or c == "":
        return (0, 0)
    if not c.startswith("="):
        try:
            return (int(c), 0)
        except ValueError:
            return (0, 0)
    inprog.add(k)
    toks = tokenize(c[1:])
    p = [0]
    def peek():
        return toks[p[0]] if p[0] < len(toks) else (None, None)
    def nxt():
        t = peek()
        p[0] += 1
        return t
    def expr():
        v, f = term()
        while peek() in (("op", "+"), ("op", "-")):
            op = nxt()[1]
            v2, f2 = term()
            v = v + v2 if op == "+" else v - v2
            f |= f2
        return v, f
    def term():
        v, f = unary()
        while peek() in (("op", "*"), ("op", "/")):
            op = nxt()[1]
            v2, f2 = unary()
            f |= f2
            if op == "*":
                v = v * v2
            else:
                if v2 == 0:
                    f |= DIV
                    v = 0
                else:
                    v = tdiv(v, v2)
        return v, f
    def unary():
        if peek() == ("op", "-"):
            nxt()
            v, f = unary()
            return -v, f
        if peek() == ("op", "+"):
            nxt()
            return unary()
        return atom()
    def ref(name):
        m = re.fullmatch(r'([A-Z])(\d+)', name)
        return ord(m.group(1)) - 64, int(m.group(2))
    def atom():
        t, v = nxt()
        if t == "n":
            return v, 0
        if t == "op" and v == "(":
            r = expr()
            nxt()
            return r
        if t == "id":
            if peek() == ("op", "(") :
                nxt()
                a = nxt()[1]
                c1, r1 = ref(a)
                nxt()  # :
                b = nxt()[1]
                c2, r2 = ref(b)
                nxt()  # )
                vals = []
                f = 0
                for cc in range(min(c1, c2), max(c1, c2) + 1):
                    for rr in range(min(r1, r2), max(r1, r2) + 1):
                        x, ff = cellval(cc, rr)
                        vals.append(x)
                        f |= ff
                if v == "SUM":
                    return sum(vals), f
                if v == "MIN":
                    return min(vals), f
                return max(vals), f
            cc, rr = ref(v)
            return cellval(cc, rr)
        return 0, 0
    r = expr()
    inprog.discard(k)
    memo[k] = r
    return r

out = ["\t" + "\t".join(chr(64 + c) for c in range(1, maxc + 1))]
for r in range(1, maxr + 1):
    row = [str(r)]
    for c in range(1, maxc + 1):
        if (c, r) not in cells or cells[(c, r)] == "":
            row.append("")
            continue
        v, f = cellval(c, r)
        if f & CYC:
            row.append("#CYCLE")
        elif f & DIV:
            row.append("#DIV0")
        else:
            row.append(str(v))
    out.append("\t".join(row))
print("\n".join(out))
