import re
import sys

TOKEN = re.compile(r"\s*(?:(\d+)|([A-Z][0-9]+)|([A-Z]+)|(.))")
REF_SCAN = re.compile(r"([A-Z])([0-9]+)")
DIV0 = "#DIV0"
CYCLE = "#CYCLE"

cells = {}   # (col, row) -> content
memo = {}
state = {}   # (col, row) -> 1 in progress, 2 done


def tokenize(s):
    toks = []
    pos = 0
    s = s.rstrip()
    while pos < len(s):
        m = TOKEN.match(s, pos)
        if m is None or m.end() == pos:
            break
        pos = m.end()
        if m.group(1):
            toks.append(("num", int(m.group(1))))
        elif m.group(2):
            toks.append(("cell", m.group(2)[0], int(m.group(2)[1:])))
        elif m.group(3):
            toks.append(("name", m.group(3)))
        elif m.group(4) and not m.group(4).isspace():
            toks.append(("op", m.group(4)))
    return toks


class Parser:
    def __init__(self, toks):
        self.toks = toks
        self.pos = 0

    def peek(self):
        return self.toks[self.pos] if self.pos < len(self.toks) else None

    def take(self):
        tok = self.toks[self.pos]
        self.pos += 1
        return tok

    def is_op(self, ch):
        t = self.peek()
        return t is not None and t[0] == "op" and t[1] == ch

    def expr(self):
        node = self.term()
        while self.is_op("+") or self.is_op("-"):
            op = self.take()[1]
            node = ("bin", op, node, self.term())
        return node

    def term(self):
        node = self.unary()
        while self.is_op("*") or self.is_op("/"):
            op = self.take()[1]
            node = ("bin", op, node, self.unary())
        return node

    def unary(self):
        if self.is_op("-"):
            self.take()
            return ("neg", self.unary())
        if self.is_op("+"):
            self.take()
            return self.unary()
        return self.primary()

    def primary(self):
        t = self.take()
        if t[0] == "num":
            return ("num", t[1])
        if t[0] == "cell":
            return ("ref", t[1], t[2])
        if t[0] == "name":
            self.take()  # '('
            a = self.take()
            self.take()  # ':'
            b = self.take()
            self.take()  # ')'
            r1, r2 = sorted([a[2], b[2]])
            c1, c2 = sorted([a[1], b[1]])
            return ("func", t[1], c1, r1, c2, r2)
        # parenthesised expression
        node = self.expr()
        self.take()  # ')'
        return node


def combine(errs):
    if CYCLE in errs:
        return CYCLE
    if DIV0 in errs:
        return DIV0
    return None


def get_cell(key):
    if key not in cells:
        return 0
    if state.get(key) == 1:
        return CYCLE
    if state.get(key) == 2:
        return memo[key]
    state[key] = 1
    content = cells[key]
    if content.startswith("="):
        node = Parser(tokenize(content[1:])).expr()
        val = evaluate(node)
    else:
        val = int(content)
    memo[key] = val
    state[key] = 2
    return val


def evaluate(node):
    kind = node[0]
    if kind == "num":
        return node[1]
    if kind == "ref":
        return get_cell((node[1], node[2]))
    if kind == "neg":
        v = evaluate(node[1])
        return v if isinstance(v, str) else -v
    if kind == "bin":
        op = node[1]
        l = evaluate(node[2])
        r = evaluate(node[3])
        err = combine([x for x in (l, r) if isinstance(x, str)])
        if err:
            return err
        if op == "+":
            return l + r
        if op == "-":
            return l - r
        if op == "*":
            return l * r
        if r == 0:
            return DIV0
        q = abs(l) // abs(r)
        return q if (l >= 0) == (r >= 0) else -q
    if kind == "func":
        _, name, c1, r1, c2, r2 = node
        vals = []
        for c in range(ord(c1), ord(c2) + 1):
            for r in range(r1, r2 + 1):
                vals.append(get_cell((chr(c), r)))
        err = combine([v for v in vals if isinstance(v, str)])
        if err:
            return err
        if name == "SUM":
            return sum(vals)
        if name == "MIN":
            return min(vals)
        return max(vals)
    raise ValueError(kind)


def main():
    for raw in sys.stdin.read().splitlines():
        if "=" not in raw:
            continue
        name, _, content = raw.partition("=")
        name = name.strip()
        if not re.fullmatch(r"[A-Z][0-9]+", name):
            continue
        content = content.strip()
        cells[(name[0], int(name[1:]))] = content

    # determine the grid extent
    refs = list(cells.keys())
    for content in cells.values():
        for col, row in REF_SCAN.findall(content):
            refs.append((col, int(row)))
    if refs:
        last_col = max(c for c, _ in refs)
        last_row = max(r for _, r in refs)
    else:
        last_col, last_row = "A", 1

    cols = [chr(c) for c in range(ord("A"), ord(last_col) + 1)]
    out = ["\t" + "\t".join(cols)]
    for r in range(1, last_row + 1):
        vals = []
        for c in cols:
            key = (c, r)
            if key not in cells:
                vals.append("")
                continue
            v = get_cell(key)
            vals.append(str(v))
        out.append(str(r) + "\t" + "\t".join(vals))
    sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
