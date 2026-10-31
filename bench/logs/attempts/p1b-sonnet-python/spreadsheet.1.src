import sys, re

sys.setrecursionlimit(100000)
raw = {}
maxc = 0
maxr = 0
ref_re = re.compile(r'^([A-Z])(\d+)$')


def touch(c, r):
    global maxc, maxr
    maxc = max(maxc, c)
    maxr = max(maxr, r)


def tokenize(f):
    f = re.sub(r'\s+', '', f)
    toks = re.findall(r'\d+|[A-Z]+\d+|[A-Z]+|[-+*/():]|.', f)
    return toks


for line in sys.stdin.read().split("\n"):
    if "=" not in line:
        continue
    cell, _, content = line.partition("=")
    cell = cell.strip()
    content = content.strip()
    m = ref_re.match(cell)
    if not m:
        continue
    c = ord(m.group(1)) - 64
    r = int(m.group(2))
    touch(c, r)
    if content.startswith("="):
        toks = tokenize(content[1:])
        raw[(c, r)] = ("f", toks)
        for t in toks:
            mm = ref_re.match(t)
            if mm:
                touch(ord(mm.group(1)) - 64, int(mm.group(2)))
    else:
        try:
            raw[(c, r)] = ("n", int(content))
        except ValueError:
            raw[(c, r)] = ("n", 0)

memo = {}
CYC = "C"
DIV = "D"


def cell_val(key):
    if key not in raw:
        return (0, frozenset())
    if key in memo:
        v = memo[key]
        if v is None:
            return (0, frozenset([CYC]))
        return v
    kind, d = raw[key]
    if kind == "n":
        memo[key] = (d, frozenset())
        return memo[key]
    memo[key] = None
    p = Parser(d)
    v = p.expr()
    memo[key] = v
    return v


def idiv(a, b):
    q = abs(a) // abs(b)
    return q if (a < 0) == (b < 0) else -q


class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else None

    def nxt(self):
        v = self.peek()
        self.i += 1
        return v

    def expr(self):
        v, fl = self.term()
        while self.peek() in ("+", "-"):
            op = self.nxt()
            v2, f2 = self.term()
            v = v + v2 if op == "+" else v - v2
            fl = fl | f2
        return (v, fl)

    def term(self):
        v, fl = self.unary()
        while self.peek() in ("*", "/"):
            op = self.nxt()
            v2, f2 = self.unary()
            fl = fl | f2
            if op == "*":
                v = v * v2
            else:
                if v2 == 0:
                    fl = fl | frozenset([DIV])
                    v = 0
                else:
                    v = idiv(v, v2)
        return (v, fl)

    def unary(self):
        if self.peek() == "-":
            self.nxt()
            v, fl = self.unary()
            return (-v, fl)
        if self.peek() == "+":
            self.nxt()
            return self.unary()
        return self.atom()

    def atom(self):
        t = self.nxt()
        if t is None:
            return (0, frozenset())
        if t == "(":
            v = self.expr()
            if self.peek() == ")":
                self.nxt()
            return v
        if t.isdigit():
            return (int(t), frozenset())
        m = ref_re.match(t)
        if m:
            return cell_val((ord(m.group(1)) - 64, int(m.group(2))))
        if t in ("SUM", "MIN", "MAX"):
            if self.peek() == "(":
                self.nxt()
            a = self.nxt()
            if self.peek() == ":":
                self.nxt()
            b = self.nxt()
            if self.peek() == ")":
                self.nxt()
            ma = ref_re.match(a or "")
            mb = ref_re.match(b or "")
            if not ma or not mb:
                return (0, frozenset())
            c1, c2 = sorted((ord(ma.group(1)) - 64, ord(mb.group(1)) - 64))
            r1, r2 = sorted((int(ma.group(2)), int(mb.group(2))))
            vals = []
            fl = frozenset()
            for c in range(c1, c2 + 1):
                for r in range(r1, r2 + 1):
                    v, f = cell_val((c, r))
                    vals.append(v)
                    fl = fl | f
            if t == "SUM":
                res = sum(vals)
            elif t == "MIN":
                res = min(vals)
            else:
                res = max(vals)
            return (res, fl)
        return (0, frozenset())


header = [""] + [chr(64 + c) for c in range(1, maxc + 1)]
print("\t".join(header))
for r in range(1, maxr + 1):
    row = [str(r)]
    for c in range(1, maxc + 1):
        if (c, r) not in raw:
            row.append("")
            continue
        v, fl = cell_val((c, r))
        if CYC in fl:
            row.append("#CYCLE")
        elif DIV in fl:
            row.append("#DIV0")
        else:
            row.append(str(v))
    print("\t".join(row))
