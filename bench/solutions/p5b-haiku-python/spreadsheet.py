import re
import sys

CYCLE = "CYCLE"
DIV0 = "DIV0"

CELL_LINE_RE = re.compile(r"^\s*([A-Z])([0-9]{1,2})\s*=\s*(.*?)\s*$")
TOKEN_RE = re.compile(r"(SUM|MIN|MAX)\(|([A-Z])([0-9]{1,2})|([0-9]+)|([-+*/():])")


def col_index(letter):
    return ord(letter) - ord("A") + 1


def col_letter(idx):
    return chr(ord("A") + idx - 1)


def tokenize(text):
    tokens = []
    pos = 0
    text = re.sub(r"\s+", "", text)
    while pos < len(text):
        m = TOKEN_RE.match(text, pos)
        if not m:
            raise ValueError("bad formula")
        if m.group(1):
            tokens.append(("FUNC", m.group(1)))
        elif m.group(2):
            tokens.append(("CELL", (col_index(m.group(2)), int(m.group(3)))))
        elif m.group(4):
            tokens.append(("NUM", int(m.group(4))))
        else:
            tokens.append(("OP", m.group(5)))
        pos = m.end()
    return tokens


class Parser:
    def __init__(self, tokens):
        self.toks = tokens
        self.i = 0

    def peek(self):
        return self.toks[self.i] if self.i < len(self.toks) else (None, None)

    def take(self):
        t = self.peek()
        self.i += 1
        return t

    def expect_op(self, op):
        t = self.take()
        if t != ("OP", op):
            raise ValueError("expected " + op)

    def expr(self):
        node = self.term()
        while self.peek() in (("OP", "+"), ("OP", "-")):
            op = self.take()[1]
            node = ("bin", op, node, self.term())
        return node

    def term(self):
        node = self.unary()
        while self.peek() in (("OP", "*"), ("OP", "/")):
            op = self.take()[1]
            node = ("bin", op, node, self.unary())
        return node

    def unary(self):
        if self.peek() == ("OP", "-"):
            self.take()
            return ("neg", self.unary())
        if self.peek() == ("OP", "+"):
            self.take()
            return self.unary()
        return self.primary()

    def primary(self):
        kind, val = self.take()
        if kind == "NUM":
            return ("num", val)
        if kind == "CELL":
            return ("ref", val)
        if kind == "FUNC":
            k1, c1 = self.take()
            self.expect_op(":")
            k2, c2 = self.take()
            if k1 != "CELL" or k2 != "CELL":
                raise ValueError("bad range")
            self.expect_op(")")
            return ("func", val, c1, c2)
        if kind == "OP" and val == "(":
            node = self.expr()
            self.expect_op(")")
            return node
        raise ValueError("bad token")


def combine(*vals):
    if any(v == CYCLE for v in vals):
        return CYCLE
    for v in vals:
        if v == DIV0:
            return DIV0
    return None


def main():
    raw = {}  # (col,row) -> content string
    for line in sys.stdin.read().splitlines():
        m = CELL_LINE_RE.match(line)
        if not m:
            continue
        key = (col_index(m.group(1)), int(m.group(2)))
        raw[key] = m.group(3)

    cells = {}  # (col,row) -> ('value', int) or ('formula', ast)
    max_col = 1
    max_row = 1
    used_any = False

    def touch(c, r):
        nonlocal max_col, max_row, used_any
        used_any = True
        max_col = max(max_col, c)
        max_row = max(max_row, r)

    for key, content in raw.items():
        touch(*key)
        if content.startswith("="):
            tokens = tokenize(content[1:])
            p = Parser(tokens)
            ast = p.expr()
            cells[key] = ("formula", ast)
            collect_refs(ast, touch)
        else:
            cells[key] = ("value", int(content))

    memo = {}
    stack = set()

    def eval_cell(key):
        if key not in cells:
            return 0
        if key in memo:
            return memo[key]
        if key in stack:
            return CYCLE
        kind, payload = cells[key]
        if kind == "value":
            memo[key] = payload
            return payload
        stack.add(key)
        result = eval_node(payload)
        stack.discard(key)
        memo[key] = result
        return result

    def eval_node(node):
        kind = node[0]
        if kind == "num":
            return node[1]
        if kind == "ref":
            return eval_cell(node[1])
        if kind == "neg":
            v = eval_node(node[1])
            return v if isinstance(v, str) else -v
        if kind == "bin":
            op = node[1]
            a = eval_node(node[2])
            b = eval_node(node[3])
            err = combine(a, b)
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
            _, (c1, r1), (c2, r2) = node
            c_lo, c_hi = min(c1, c2), max(c1, c2)
            r_lo, r_hi = min(r1, r2), max(r1, r2)
            vals = []
            for c in range(c_lo, c_hi + 1):
                for r in range(r_lo, r_hi + 1):
                    vals.append(eval_cell((c, r)))
            err = combine(*vals)
            if err:
                return err
            name = node[0]
            return None
        raise ValueError("bad node")

    # functions need the name, handled separately to keep node layout simple
    def eval_func(name, c1, r1, c2, r2):
        c_lo, c_hi = min(c1, c2), max(c1, c2)
        r_lo, r_hi = min(r1, r2), max(r1, r2)
        vals = []
        for c in range(c_lo, c_hi + 1):
            for r in range(r_lo, r_hi + 1):
                vals.append(eval_cell((c, r)))
        err = combine(*vals)
        if err:
            return err
        if not vals:
            return 0
        if name == "SUM":
            return sum(vals)
        if name == "MIN":
            return min(vals)
        return max(vals)

    def eval_node_full(node):
        if node[0] == "func":
            return eval_func(node[1], *node[2], *node[3])
        if node[0] == "neg":
            v = eval_node_full(node[1])
            return v if isinstance(v, str) else -v
        if node[0] == "bin":
            op = node[1]
            a = eval_node_full(node[2])
            b = eval_node_full(node[3])
            err = combine(a, b)
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
        return eval_node(node)

    def eval_cell_full(key):
        if key not in cells:
            return 0
        if key in memo:
            return memo[key]
        if key in stack:
            return CYCLE
        kind, payload = cells[key]
        if kind == "value":
            memo[key] = payload
            return payload
        stack.add(key)
        result = eval_node_full(payload)
        stack.discard(key)
        memo[key] = result
        return result

    eval_cell = eval_cell_full  # noqa: F811 - used by eval_func

    if not used_any:
        return

    out = []
    header = "\t" + "\t".join(col_letter(c) for c in range(1, max_col + 1))
    out.append(header)
    for r in range(1, max_row + 1):
        row = [str(r)]
        for c in range(1, max_col + 1):
            key = (c, r)
            if key not in cells:
                row.append("")
                continue
            v = eval_cell_full(key)
            if isinstance(v, str):
                row.append("#" + v)
            else:
                row.append(str(v))
        out.append("\t".join(row))
    sys.stdout.write("\n".join(out) + "\n")


def collect_refs(node, touch):
    kind = node[0]
    if kind == "ref":
        touch(*node[1])
    elif kind == "func":
        touch(*node[2])
        touch(*node[3])
    elif kind == "neg":
        collect_refs(node[1], touch)
    elif kind == "bin":
        collect_refs(node[2], touch)
        collect_refs(node[3], touch)


if __name__ == "__main__":
    main()
