import re
import sys

sys.setrecursionlimit(100000)

TOKEN_RE = re.compile(r'([A-Z]+)(?=\()|([A-Z])(\d+)|(\d+)|([-+*/():])')

defined = {}   # (col letter, row) -> content string
cache = {}     # (col letter, row) -> int or error string
visiting = set()
mentioned = set()


def combine(errs):
    return 'CYCLE' if 'CYCLE' in errs else 'DIV0'


def binop(op, a, b):
    if isinstance(a, str) or isinstance(b, str):
        errs = [x for x in (a, b) if isinstance(x, str)]
        return combine(errs)
    if op == '+':
        return a + b
    if op == '-':
        return a - b
    if op == '*':
        return a * b
    if b == 0:
        return 'DIV0'
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


def aggregate(fn, vals):
    errs = [v for v in vals if isinstance(v, str)]
    if errs:
        return combine(errs)
    if not vals:
        return 0
    if fn == 'SUM':
        return sum(vals)
    if fn == 'MIN':
        return min(vals)
    if fn == 'MAX':
        return max(vals)
    return 0


def range_cells(a, b):
    c1, c2 = sorted([a[0], b[0]])
    r1, r2 = sorted([a[1], b[1]])
    return [(chr(c), r) for c in range(ord(c1), ord(c2) + 1)
            for r in range(r1, r2 + 1)]


def cell_value(key):
    if key not in defined:
        return 0
    if key in cache:
        return cache[key]
    if key in visiting:
        return 'CYCLE'
    visiting.add(key)
    content = defined[key]
    if content.startswith('='):
        toks = tokenize(content[1:])
        v = Parser(toks).expr()
    elif re.fullmatch(r'-?\d+', content):
        v = int(content)
    else:
        v = 0
    visiting.discard(key)
    cache[key] = v
    return v


def tokenize(text):
    s = re.sub(r'\s+', '', text)
    toks = []
    pos = 0
    while pos < len(s):
        m = TOKEN_RE.match(s, pos)
        if not m:
            pos += 1
            continue
        pos = m.end()
        if m.group(1):
            toks.append(('FUNC', m.group(1)))
        elif m.group(2):
            key = (m.group(2), int(m.group(3)))
            mentioned.add(key)
            toks.append(('REF', key))
        elif m.group(4):
            toks.append(('NUM', int(m.group(4))))
        else:
            toks.append(('OP', m.group(5)))
    return toks


class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else None

    def next(self):
        tok = self.peek()
        self.i += 1
        return tok

    def expr(self):
        v = self.term()
        while self.peek() in (('OP', '+'), ('OP', '-')):
            op = self.next()[1]
            v = binop(op, v, self.term())
        return v

    def term(self):
        v = self.unary()
        while self.peek() in (('OP', '*'), ('OP', '/')):
            op = self.next()[1]
            v = binop(op, v, self.unary())
        return v

    def unary(self):
        if self.peek() == ('OP', '-'):
            self.next()
            x = self.unary()
            return x if isinstance(x, str) else -x
        if self.peek() == ('OP', '+'):
            self.next()
            return self.unary()
        return self.primary()

    def primary(self):
        tok = self.next()
        if tok is None:
            return 0
        kind, val = tok
        if kind == 'NUM':
            return val
        if kind == 'REF':
            return cell_value(val)
        if kind == 'FUNC':
            self.next()  # '('
            first = self.next()
            last = first
            if self.peek() == ('OP', ':'):
                self.next()
                last = self.next()
            self.next()  # ')'
            cells = range_cells(first[1], last[1])
            return aggregate(val, [cell_value(c) for c in cells])
        if kind == 'OP' and val == '(':
            v = self.expr()
            self.next()  # ')'
            return v
        return 0


def main():
    for raw in sys.stdin.read().splitlines():
        if '=' not in raw:
            continue
        left, _, right = raw.partition('=')
        cell = re.sub(r'\s', '', left)
        m = re.fullmatch(r'([A-Z])(\d+)', cell)
        if not m:
            continue
        key = (m.group(1), int(m.group(2)))
        defined[key] = right.strip()
        if right.strip().startswith('='):
            tokenize(right.strip()[1:])

    keys = list(defined.keys()) + list(mentioned)
    if not keys:
        return
    max_col = max(ord(c) for c, _ in keys)
    max_row = max(r for _, r in keys)
    letters = [chr(c) for c in range(65, max_col + 1)]

    lines = ["\t".join([""] + letters)]
    for r in range(1, max_row + 1):
        vals = []
        for col in letters:
            key = (col, r)
            if key not in defined:
                vals.append("")
                continue
            v = cell_value(key)
            if isinstance(v, str):
                vals.append("#" + v)
            else:
                vals.append(str(v))
        lines.append("\t".join([str(r)] + vals))
    sys.stdout.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
