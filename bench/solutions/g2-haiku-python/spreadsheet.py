import re
import sys

TOKEN_RE = re.compile(r"(SUM|MIN|MAX)|([A-Z])(\d+)|(\d+)|([-+*/():,:])")
REF_RE = re.compile(r"^([A-Z])(\d+)$")


def tokenize(src):
    src = re.sub(r"\s+", "", src)
    tokens = []
    pos = 0
    while pos < len(src):
        m = TOKEN_RE.match(src, pos)
        if not m:
            raise ValueError("bad formula near %r" % src[pos:])
        if m.group(1):
            tokens.append(("func", m.group(1)))
        elif m.group(2):
            tokens.append(("ref", (m.group(2), int(m.group(3)))))
        elif m.group(4):
            tokens.append(("num", int(m.group(4))))
        else:
            tokens.append(("op", m.group(5)))
        pos = m.end()
    return tokens


def idiv(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


class Sheet:
    def __init__(self, raw):
        self.raw = raw  # (col, row) -> content string
        self.values = {}
        self.state = {}  # (col, row) -> "busy" or "done"
        self.cache = {}

    def cell_value(self, key):
        """Return ("ok", int) or ("err", "CYCLE"|"DIV0")."""
        if key in self.cache:
            return self.cache[key]
        if self.state.get(key) == "busy":
            return ("err", "CYCLE")
        content = self.raw.get(key)
        if content is None:
            return ("ok", 0)
        if not content.startswith("="):
            res = ("ok", int(content))
            self.cache[key] = res
            return res
        self.state[key] = "busy"
        res = self.eval_formula(content[1:])
        self.state[key] = "done"
        self.cache[key] = res
        return res

    def eval_formula(self, src):
        tokens = tokenize(src)
        flags = {"cycle": False, "div": False}
        p = Parser(tokens, self, flags)
        val = p.parse_expr()
        if p.pos != len(tokens):
            raise ValueError("trailing tokens")
        if flags["cycle"]:
            return ("err", "CYCLE")
        if flags["div"]:
            return ("err", "DIV0")
        return ("ok", val)


class Parser:
    def __init__(self, tokens, sheet, flags):
        self.t = tokens
        self.pos = 0
        self.sheet = sheet
        self.flags = flags

    def peek(self):
        return self.t[self.pos] if self.pos < len(self.t) else (None, None)

    def take(self):
        tok = self.peek()
        self.pos += 1
        return tok

    def parse_expr(self):
        val = self.parse_term()
        while self.peek() in (("op", "+"), ("op", "-")):
            op = self.take()[1]
            rhs = self.parse_term()
            val = val + rhs if op == "+" else val - rhs
        return val

    def parse_term(self):
        val = self.parse_unary()
        while self.peek() in (("op", "*"), ("op", "/")):
            op = self.take()[1]
            rhs = self.parse_unary()
            if op == "*":
                val = val * rhs
            else:
                if rhs == 0:
                    self.flags["div"] = True
                    val = 0
                else:
                    val = idiv(val, rhs)
        return val

    def parse_unary(self):
        if self.peek() == ("op", "-"):
            self.take()
            return -self.parse_unary()
        if self.peek() == ("op", "+"):
            self.take()
            return self.parse_unary()
        return self.parse_primary()

    def ref_value(self, key):
        kind, val = self.sheet.cell_value(key)
        if kind == "ok":
            return val
        if val == "CYCLE":
            self.flags["cycle"] = True
        else:
            self.flags["div"] = True
        return 0

    def parse_primary(self):
        kind, val = self.take()
        if kind == "num":
            return val
        if kind == "ref":
            return self.ref_value(val)
        if kind == "op" and val == "(":
            inner = self.parse_expr()
            if self.take() != ("op", ")"):
                raise ValueError("expected )")
            return inner
        if kind == "func":
            if self.take() != ("op", "("):
                raise ValueError("expected (")
            a = self.take()
            if a[0] != "ref":
                raise ValueError("expected range")
            if self.take() != ("op", ":"):
                raise ValueError("expected :")
            b = self.take()
            if b[0] != "ref":
                raise ValueError("expected range end")
            if self.take() != ("op", ")"):
                raise ValueError("expected )")
            (c1, r1), (c2, r2) = a[1], b[1]
            cols = range(min(ord(c1), ord(c2)), max(ord(c1), ord(c2)) + 1)
            rows = range(min(r1, r2), max(r1, r2) + 1)
            vals = []
            for c in cols:
                for r in rows:
                    vals.append(self.ref_value((chr(c), r)))
            if not vals:
                return 0
            if val == "SUM":
                return sum(vals)
            if val == "MIN":
                return min(vals)
            return max(vals)
        raise ValueError("unexpected token")


def main():
    raw = {}
    mentioned = []
    for line in sys.stdin:
        line = line.rstrip("\r\n")
        if not line.strip():
            continue
        left, _, content = line.partition("=")
        cell = left.strip()
        content = content.strip()
        m = REF_RE.match(cell)
        if not m:
            continue
        key = (m.group(1), int(m.group(2)))
        raw[key] = content
        mentioned.append(key)

    # Collect all cells mentioned in formulas.
    for key, content in raw.items():
        if content.startswith("="):
            for kind, val in tokenize(content[1:]):
                if kind == "ref":
                    mentioned.append(val)
                    # ranges: endpoints are both mentioned; include whole rectangle
    # Expand ranges (both endpoints appear as ref tokens; rectangle bounds
    # are determined by their min/max, so just use endpoints for extent).
    cols = [k[0] for k in mentioned]
    rows = [k[1] for k in mentioned]
    if not mentioned:
        return
    last_col = max(cols)
    last_row = max(rows)
    sheet = Sheet(raw)

    header = "\t" + "\t".join(chr(c) for c in range(ord("A"), ord(last_col) + 1))
    lines = [header]
    for r in range(1, last_row + 1):
        row_vals = [str(r)]
        for c in range(ord("A"), ord(last_col) + 1):
            kind, val = sheet.cell_value((chr(c), r))
            if kind == "ok":
                row_vals.append("" if (chr(c), r) not in raw and val == 0 else str(val))
            else:
                row_vals.append("#" + val)
        lines.append("\t".join(row_vals))
    sys.stdout.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
