import re
import sys

CYCLE = "#CYCLE"
DIV0 = "#DIV0"
ERRORS = (CYCLE, DIV0)

LINE_RE = re.compile(r"^\s*([A-Z])([0-9]{1,2})\s*=\s*(.*?)\s*$")
TOKEN_RE = re.compile(r"(SUM|MIN|MAX)|([A-Z])([0-9]{1,2})|([0-9]+)|([-+*/():,])")
REF_RE = re.compile(r"[A-Z][0-9]{1,2}")


def tokenize(src):
    src = re.sub(r"\s+", "", src)
    toks = []
    pos = 0
    while pos < len(src):
        m = TOKEN_RE.match(src, pos)
        if not m:
            raise ValueError("bad formula")
        if m.group(1):
            toks.append(("func", m.group(1)))
        elif m.group(2):
            toks.append(("ref", (m.group(2), int(m.group(3)))))
        elif m.group(4):
            toks.append(("num", int(m.group(4))))
        else:
            toks.append(("op", m.group(5)))
        pos = m.end()
    return toks


class Parser:
    def __init__(self, toks):
        self.toks = toks
        self.i = 0

    def peek(self):
        return self.toks[self.i] if self.i < len(self.toks) else (None, None)

    def take(self):
        t = self.peek()
        self.i += 1
        return t

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
            return ("num", val)
        if kind == "ref":
            return ("ref", val)
        if kind == "func":
            self.take()  # (
            c1, r1 = self.take()[1]
            self.take()  # :
            c2, r2 = self.take()[1]
            self.take()  # )
            return ("func", val, (c1, r1), (c2, r2))
        if kind == "op" and val == "(":
            node = self.expr()
            self.take()  # )
            return node
        raise ValueError("bad formula")


def col_index(letter):
    return ord(letter) - ord("A")


def main():
    cells = {}  # (col, row) -> int or ("f", ast)
    refs = []
    for raw in sys.stdin.read().splitlines():
        m = LINE_RE.match(raw)
        if not m:
            continue
        coord = (m.group(1), int(m.group(2)))
        content = m.group(3)
        if content.startswith("="):
            toks = tokenize(content[1:])
            ast = Parser(toks).expr()
            cells[coord] = ("f", ast)
            for t in toks:
                if t[0] == "ref":
                    refs.append(t[1])

        elif re.fullmatch(r"-?[0-9]+", content):
            cells[coord] = int(content)
        else:
            cells[coord] = 0
        cells.setdefault(coord, 0)

    # collect every referenced coordinate, including range endpoints
    used = set(cells.keys())
    used.update(refs)

    def collect_ranges(node):
        if node[0] == "func":
            used.add(node[2])
            used.add(node[3])
        elif node[0] == "bin":
            collect_ranges(node[2])
            collect_ranges(node[3])
        elif node[0] == "neg":
            collect_ranges(node[1])
    for v in list(cells.values()):
        if isinstance(v, tuple):
            collect_ranges(v[1])

    if not used:
        return

    max_col = max(col_index(c) for c, _ in used)
    max_row = max(r for _, r in used)

    memo = {}
    busy = set()

    def combine(vals):
        if CYCLE in vals:
            return CYCLE
        if DIV0 in vals:
            return DIV0
        return None

    def cell_value(coord):
        if coord in memo:
            return memo[coord]
        if coord in busy:
            return CYCLE
        if coord not in cells:
            return 0
        busy.add(coord)
        entry = cells[coord]
        if isinstance(entry, tuple):
            v = ev(entry[1])
        else:
            v = entry
        busy.discard(coord)
        memo[coord] = v
        return v

    def ev(node):
        kind = node[0]
        if kind == "num":
            return node[1]
        if kind == "ref":
            return cell_value(node[1])
        if kind == "neg":
            x = ev(node[1])
            if x in ERRORS:
                return x
            return -x
        if kind == "bin":
            op = node[1]
            a = ev(node[2])
            b = ev(node[3])
            err = combine([a, b])
            if err:
                return err
            if op == "+":
                return a + b
            if op == "-":
                return a - b
            if op == "*":
                return a * b
            if b == 0:
                return DIV0
            q = abs(a) // abs(b)
            return q if (a < 0) == (b < 0) else -q
        if kind == "func":
            c1, r1 = node[2]
            c2, r2 = node[3]
            cols = range(min(col_index(c1), col_index(c2)), max(col_index(c1), col_index(c2)) + 1)
            rows = range(min(r1, r2), max(r1, r2) + 1)
            vals = [cell_value((chr(ord("A") + c), r)) for c in cols for r in rows]
            err = combine(vals)
            if err:
                return err
            name = node[1]
            if name == "SUM":
                return sum(vals)
            if name == "MIN":
                return min(vals)
            return max(vals)
        raise ValueError("bad node")

    lines = ["\t" + "\t".join(chr(ord("A") + c) for c in range(max_col + 1))]
    for row in range(1, max_row + 1):
        vals = []
        for c in range(max_col + 1):
            coord = (chr(ord("A") + c), row)
            if coord not in cells:
                vals.append("")
            else:
                vals.append(str(cell_value(coord)))
        lines.append(str(row) + "\t" + "\t".join(vals))
    sys.stdout.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
