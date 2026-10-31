import re
import sys
import threading

TOKEN_RE = re.compile(r'SUM\(|MIN\(|MAX\(|[A-Z][0-9]+|[0-9]+|[-+*/():]')


def tokenize(src):
    return TOKEN_RE.findall(re.sub(r'\s+', '', src))


def cref(tok):
    return (tok[0], int(tok[1:]))


class Parser:
    def __init__(self, toks):
        self.t = toks
        self.p = 0

    def peek(self):
        return self.t[self.p] if self.p < len(self.t) else None

    def take(self):
        tok = self.t[self.p]
        self.p += 1
        return tok

    def expr(self):
        node = self.term()
        while self.peek() in ('+', '-'):
            op = self.take()
            node = ('bin', op, node, self.term())
        return node

    def term(self):
        node = self.unary()
        while self.peek() in ('*', '/'):
            op = self.take()
            node = ('bin', op, node, self.unary())
        return node

    def unary(self):
        if self.peek() == '-':
            self.take()
            return ('neg', self.unary())
        return self.primary()

    def primary(self):
        tok = self.take()
        if tok == '(':
            node = self.expr()
            if self.peek() == ')':
                self.take()
            return node
        if tok.endswith('('):
            a = self.take()
            b = a
            if self.peek() == ':':
                self.take()
                b = self.take()
            if self.peek() == ')':
                self.take()
            return ('func', tok[:-1], cref(a), cref(b))
        if tok[0].isdigit():
            return ('num', int(tok))
        return ('ref', tok[0], int(tok[1:]))


def combine(vals):
    errs = [v for v in vals if isinstance(v, str)]
    return 'CYCLE' if 'CYCLE' in errs else 'DIV0'


class Sheet:
    def __init__(self, cells):
        self.cells = cells
        self.memo = {}
        self.visiting = set()

    def value(self, key):
        if key in self.memo:
            return self.memo[key]
        if key in self.visiting:
            return 'CYCLE'
        content = self.cells.get(key)
        if content is None:
            return 0
        if isinstance(content, int):
            v = content
        else:
            self.visiting.add(key)
            v = self.ev(content)
            self.visiting.discard(key)
        self.memo[key] = v
        return v

    def ev(self, node):
        kind = node[0]
        if kind == 'num':
            return node[1]
        if kind == 'ref':
            return self.value((node[1], node[2]))
        if kind == 'neg':
            v = self.ev(node[1])
            return v if isinstance(v, str) else -v
        if kind == 'bin':
            a = self.ev(node[2])
            b = self.ev(node[3])
            if isinstance(a, str) or isinstance(b, str):
                return combine([a, b])
            op = node[1]
            if op == '+':
                return a + b
            if op == '-':
                return a - b
            if op == '*':
                return a * b
            if b == 0:
                return 'DIV0'
            q = abs(a) // abs(b)
            return q if (a < 0) == (b < 0) else -q
        # function over a range
        name = node[1]
        c1, r1 = node[2]
        c2, r2 = node[3]
        cols = range(min(ord(c1), ord(c2)), max(ord(c1), ord(c2)) + 1)
        rows = range(min(r1, r2), max(r1, r2) + 1)
        vals = [self.value((chr(c), r)) for c in cols for r in rows]
        errs = [v for v in vals if isinstance(v, str)]
        if errs:
            return combine(errs)
        if name == 'SUM':
            return sum(vals)
        if name == 'MIN':
            return min(vals)
        return max(vals)


def fmt(v):
    if isinstance(v, str):
        return '#' + v
    return str(v)


def main():
    cells = {}
    mentioned = set()
    for raw in sys.stdin.read().split('\n'):
        line = raw.strip()
        if not line:
            continue
        m = re.fullmatch(r'([A-Z])([0-9]{1,2})\s*=\s*(.*)', line)
        if not m:
            continue
        key = (m.group(1), int(m.group(2)))
        content = m.group(3).strip()
        mentioned.add(key)
        if content.startswith('='):
            src = re.sub(r'\s+', '', content[1:])
            cells[key] = Parser(tokenize(src)).expr()
            for c, r in re.findall(r'([A-Z])([0-9]+)', src):
                mentioned.add((c, int(r)))
        else:
            cells[key] = int(content)
    keys = set(cells) | mentioned
    if not keys:
        return
    last_col = max(c for c, _ in keys)
    last_row = max(r for _, r in keys)
    cols = [chr(c) for c in range(ord('A'), ord(last_col) + 1)]
    sheet = Sheet(cells)
    out = ['\t' + '\t'.join(cols)]
    for r in range(1, last_row + 1):
        row = []
        for c in cols:
            key = (c, r)
            row.append(fmt(sheet.value(key)) if key in cells else '')
        out.append(str(r) + '\t' + '\t'.join(row))
    sys.stdout.write('\n'.join(out) + '\n')


sys.setrecursionlimit(200000)
threading.stack_size(256 * 1024 * 1024)
worker = threading.Thread(target=main)
worker.start()
worker.join()
