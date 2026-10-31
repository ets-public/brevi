import re
import sys

sys.setrecursionlimit(50000)

CYC = "#CYCLE"
DIV = "#DIV0"
TOKEN = re.compile(r"(?P<range>[A-Z][0-9]+:[A-Z][0-9]+)|(?P<func>(?:SUM|MIN|MAX)\()"
                   r"|(?P<cell>[A-Z][0-9]+)|(?P<num>[0-9]+)|(?P<op>[-+*/()])")


def cell_key(col, row):
    return f"{col}{row}"


def split_ref(ref):
    m = re.fullmatch(r"([A-Z])([0-9]+)", ref)
    return m.group(1), int(m.group(2))


def expand_range(rng):
    a, b = rng.split(":")
    c1, r1 = split_ref(a)
    c2, r2 = split_ref(b)
    cols = range(min(ord(c1), ord(c2)), max(ord(c1), ord(c2)) + 1)
    rows = range(min(r1, r2), max(r1, r2) + 1)
    return [f"{chr(c)}{r}" for c in cols for r in rows]


def tokenize(text):
    text = re.sub(r"\s+", "", text)
    pos = 0
    toks = []
    while pos < len(text):
        m = TOKEN.match(text, pos)
        if not m:
            raise ValueError("bad formula")
        pos = m.end()
        kind = m.lastgroup
        toks.append((kind, m.group(kind)))
    return toks


class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else (None, None)

    def take(self):
        tok = self.peek()
        self.i += 1
        return tok

    def expr(self):
        node = self.term()
        while self.peek() in (("op", "+"), ("op", "-")):
            op = self.take()[1]
            node = ("bin", op, node, self.term())
        return node

    def term(self):
        node = self.unary()
        while self.peek() in (("op", "*"), ("op", "/")):
            op = self.take()[1]
            node = ("bin", op, node, self.unary())
        return node

    def unary(self):
        if self.peek() == ("op", "-"):
            self.take()
            return ("neg", self.unary())
        return self.primary()

    def primary(self):
        kind, val = self.take()
        if kind == "num":
            return ("num", int(val))
        if kind == "cell":
            return ("ref", val)
        if kind == "func":
            rng_kind, rng = self.take()
            if rng_kind == "cell":
                cells = [rng]
            else:
                cells = expand_range(rng)
            self.take()  # ')'
            return ("func", val[:-1], cells)
        if (kind, val) == ("op", "("):
            node = self.expr()
            self.take()  # ')'
            return node
        raise ValueError("bad formula")


def combine(*vals):
    errs = [v for v in vals if isinstance(v, str)]
    if not errs:
        return None
    return CYC if CYC in errs else DIV


def main():
    cells = {}  # key -> ("const", int) or ("formula", ast, deps)
    for raw in sys.stdin:
        line = raw.strip()
        if not line or "=" not in line:
            continue
        left, right = line.split("=", 1)
        key = left.strip()
        right = right.strip()
        if right.startswith("="):
            ast = Parser(tokenize(right[1:])).expr()
            cells[key] = ("formula", ast)
        else:
            cells[key] = ("const", int(right))

    state = {}
    memo = {}

    def value_of(key):
        if key in state and state[key] == 1:
            return CYC
        if key in memo:
            return memo[key]
        if key not in cells:
            return 0
        kind = cells[key][0]
        if kind == "const":
            memo[key] = cells[key][1]
            return memo[key]
        state[key] = 1
        v = ev(cells[key][1])
        state[key] = 2
        memo[key] = v
        return v

    def ev(node):
        t = node[0]
        if t == "num":
            return node[1]
        if t == "ref":
            return value_of(node[1])
        if t == "neg":
            v = ev(node[1])
            return v if isinstance(v, str) else -v
        if t == "func":
            vals = [value_of(k) for k in node[2]]
            e = combine(*vals)
            if e:
                return e
            if node[1] == "SUM":
                return sum(vals)
            if node[1] == "MIN":
                return min(vals)
            return max(vals)
        if t == "bin":
            op, a, b = node[1], ev(node[2]), ev(node[3])
            e = combine(a, b)
            if op == "/" and e is None and b == 0:
                return DIV
            if e:
                return e
            if op == "+":
                return a + b
            if op == "-":
                return a - b
            if op == "*":
                return a * b
            q = abs(a) // abs(b)
            return -q if (a < 0) != (b < 0) else q
        raise ValueError("bad node")

    # Determine used grid extent (defined cells and cells mentioned in formulas)
    all_refs = set(cells.keys())

    def collect(node):
        t = node[0]
        if t == "ref":
            all_refs.add(node[1])
        elif t == "func":
            all_refs.update(node[2])
        elif t in ("neg",):
            collect(node[1])
        elif t == "bin":
            collect(node[2])
            collect(node[3])

    for kind_ast in cells.values():
        if kind_ast[0] == "formula":
            collect(kind_ast[1])

    cols = [split_ref(k)[0] for k in all_refs]
    rows = [split_ref(k)[1] for k in all_refs]
    last_col = max(cols) if cols else "A"
    last_row = max(rows) if rows else 1

    col_list = [chr(c) for c in range(ord("A"), ord(last_col) + 1)]
    out = ["\t" + "\t".join(col_list)]
    for r in range(1, last_row + 1):
        vals = []
        for c in col_list:
            key = f"{c}{r}"
            if key in cells:
                v = value_of(key)
                vals.append("" if v is None else str(v))
            else:
                vals.append("")
        out.append(str(r) + "\t" + "\t".join(vals))
    print("\n".join(out))


main()
