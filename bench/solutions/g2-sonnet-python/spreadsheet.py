import sys, re
CYC, DIV = "#CYCLE", "#DIV0"
cells = {}   # (col,row) -> ast
mentioned = set()
TOK = re.compile(r"\s*(?:(\d+)|([A-Z]+\d+(?::[A-Z]+\d+)?)|([A-Z]+)|(.))")
def ref(s):
    m = re.match(r"([A-Z])(\d+)$", s)
    return (ord(m.group(1)) - 65, int(m.group(2)))
def tokenize(s):
    toks = []
    for m in TOK.finditer(s):
        if m.group(1): toks.append(("n", int(m.group(1))))
        elif m.group(2): toks.append(("r", m.group(2)))
        elif m.group(3): toks.append(("f", m.group(3)))
        elif m.group(4) and not m.group(4).isspace(): toks.append(("o", m.group(4)))
    return toks
class P:
    def __init__(self, toks): self.t = toks; self.i = 0
    def peek(self): return self.t[self.i] if self.i < len(self.t) else (None, None)
    def nxt(self): x = self.peek(); self.i += 1; return x
    def expr(self):
        n = self.term()
        while self.peek() in (("o", "+"), ("o", "-")):
            op = self.nxt()[1]; n = ("bin", op, n, self.term())
        return n
    def term(self):
        n = self.unary()
        while self.peek() in (("o", "*"), ("o", "/")):
            op = self.nxt()[1]; n = ("bin", op, n, self.unary())
        return n
    def unary(self):
        if self.peek() == ("o", "-"):
            self.nxt(); return ("neg", self.unary())
        if self.peek() == ("o", "+"):
            self.nxt(); return self.unary()
        return self.atom()
    def atom(self):
        k, v = self.nxt()
        if k == "n": return ("num", v)
        if k == "r":
            if ":" in v:
                a, b = v.split(":"); a = ref(a); b = ref(b)
                mentioned.add(a); mentioned.add(b)
                return ("range", a, b)
            r = ref(v); mentioned.add(r); return ("ref", r)
        if k == "f":
            self.nxt()  # (
            arg = self.expr_range()
            self.nxt()  # )
            return ("fn", v, arg)
        if (k, v) == ("o", "("):
            n = self.expr(); self.nxt(); return n
        raise ValueError
    def expr_range(self):
        return self.atom() if self.peek()[0] == "r" and ":" in self.peek()[1] else self.expr()
def idiv(a, b):
    q = abs(a) // abs(b)
    return q if (a < 0) == (b < 0) else -q
memo = {}
state = {}
def evalcell(c):
    if c in memo: return memo[c]
    if c not in cells: return 0
    if state.get(c) == 1: return CYC
    state[c] = 1
    r = ev(cells[c])
    state[c] = 2
    memo[c] = r
    return r
def combine(vals):
    if CYC in vals: return CYC
    if DIV in vals: return DIV
    return None
def ev(n):
    k = n[0]
    if k == "num": return n[1]
    if k == "ref": return evalcell(n[1])
    if k == "neg":
        v = ev(n[1]); return v if isinstance(v, str) else -v
    if k == "bin":
        a = ev(n[2]); b = ev(n[3])
        e = combine([a, b])
        if e: return e
        op = n[1]
        if op == "+": return a + b
        if op == "-": return a - b
        if op == "*": return a * b
        if b == 0: return DIV
        return idiv(a, b)
    if k == "fn":
        arg = n[2]
        if arg[0] == "range":
            (c1, r1), (c2, r2) = arg[1], arg[2]
            vals = [evalcell((c, r)) for c in range(min(c1, c2), max(c1, c2) + 1)
                    for r in range(min(r1, r2), max(r1, r2) + 1)]
        else:
            vals = [ev(arg)]
        e = combine(vals)
        if e: return e
        return {"SUM": sum, "MIN": min, "MAX": max}[n[1]](vals)
    return 0
for line in sys.stdin.read().split("\n"):
    if "=" not in line: continue
    l, c = line.split("=", 1)
    l = l.strip(); c = c.strip()
    if not re.match(r"[A-Z]\d+$", l): continue
    cell = ref(l); mentioned.add(cell)
    if c.startswith("="):
        cells[cell] = P(tokenize(c[1:])).expr()
    else:
        try: cells[cell] = ("num", int(c))
        except ValueError: cells[cell] = ("num", 0)
if mentioned:
    mc = max(c for c, r in mentioned); mr = max(r for c, r in mentioned)
    print("\t" + "\t".join(chr(65 + i) for i in range(mc + 1)))
    for r in range(1, mr + 1):
        row = []
        for c in range(mc + 1):
            row.append(str(evalcell((c, r))) if (c, r) in cells else "")
        print(str(r) + "\t" + "\t".join(row))
