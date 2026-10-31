import re
import sys

CYCLE = 'CYCLE'
DIV0 = 'DIV0'

TOKEN_RE = re.compile(r'\s*(?:(SUM|MIN|MAX)|([A-Z]\d+)|(\d+)|([-+*/():]))')


class ParseError(Exception):
    pass


def tokenize(s):
    s = re.sub(r'\s+', '', s)
    tokens = []
    pos = 0
    while pos < len(s):
        m = TOKEN_RE.match(s, pos)
        if not m or m.end() == pos:
            raise ParseError('bad token at %d' % pos)
        pos = m.end()
        if m.group(1):
            tokens.append(('func', m.group(1)))
        elif m.group(2):
            tokens.append(('ref', m.group(2)))
        elif m.group(3):
            tokens.append(('num', int(m.group(3))))
        else:
            tokens.append(('op', m.group(4)))
    return tokens


def split_ref(ref):
    col = ord(ref[0]) - ord('A')
    row = int(ref[1:])
    return col, row


def combine_err(vals):
    """Return the error marker to propagate (CYCLE beats DIV0), or None."""
    if any(v == CYCLE for v in vals):
        return CYCLE
    if any(v == DIV0 for v in vals):
        return DIV0
    return None


class Sheet:
    def __init__(self, cells):
        self.raw = cells          # (col,row) -> ('int', n) or ('formula', tokens)
        self.memo = {}
        self.visiting = set()

    def value(self, key):
        if key not in self.raw:
            return 0
        if key in self.memo:
            return self.memo[key]
        if key in self.visiting:
            return CYCLE
        kind, payload = self.raw[key]
        if kind == 'int':
            self.memo[key] = payload
            return payload
        self.visiting.add(key)
        try:
            val = self.eval_formula(payload)
        finally:
            self.visiting.discard(key)
        self.memo[key] = val
        return val

    def eval_formula(self, tokens):
        # Parser state is saved and restored because evaluation recurses
        # into other formulas through self.value().
        saved = (getattr(self, 'toks', None), getattr(self, 'p', 0))
        self.toks = tokens
        self.p = 0
        try:
            v = self.parse_expr()
            if self.p != len(self.toks):
                raise ParseError('trailing tokens')
            return v
        except ParseError:
            return 0
        finally:
            self.toks, self.p = saved

    def peek(self):
        return self.toks[self.p] if self.p < len(self.toks) else (None, None)

    def parse_expr(self):
        left = self.parse_term()
        while self.peek() in (('op', '+'), ('op', '-')):
            op = self.toks[self.p][1]
            self.p += 1
            right = self.parse_term()
            err = combine_err([left, right])
            if err:
                left = err
            elif op == '+':
                left = left + right
            else:
                left = left - right
        return left

    def parse_term(self):
        left = self.parse_unary()
        while self.peek() in (('op', '*'), ('op', '/')):
            op = self.toks[self.p][1]
            self.p += 1
            right = self.parse_unary()
            err = combine_err([left, right])
            if err:
                left = err
            elif op == '*':
                left = left * right
            else:
                if right == 0:
                    left = DIV0
                else:
                    q = abs(left) // abs(right)
                    left = q if (left >= 0) == (right > 0) else -q
        return left

    def parse_unary(self):
        if self.peek() == ('op', '-'):
            self.p += 1
            v = self.parse_unary()
            return v if isinstance(v, str) else -v
        if self.peek() == ('op', '+'):
            self.p += 1
            return self.parse_unary()
        return self.parse_primary()

    def parse_primary(self):
        kind, val = self.peek()
        if kind == 'num':
            self.p += 1
            return val
        if kind == 'ref':
            self.p += 1
            return self.value(split_ref(val))
        if kind == 'func':
            self.p += 1
            return self.parse_func(val)
        if (kind, val) == ('op', '('):
            self.p += 1
            v = self.parse_expr()
            if self.peek() != ('op', ')'):
                raise ParseError('expected )')
            self.p += 1
            return v
        raise ParseError('unexpected token')

    def parse_func(self, name):
        if self.peek() != ('op', '('):
            raise ParseError('expected (')
        self.p += 1
        if self.peek()[0] != 'ref':
            raise ParseError('expected ref')
        a = split_ref(self.toks[self.p][1])
        self.p += 1
        b = a
        if self.peek() == ('op', ':'):
            self.p += 1
            if self.peek()[0] != 'ref':
                raise ParseError('expected ref')
            b = split_ref(self.toks[self.p][1])
            self.p += 1
        if self.peek() != ('op', ')'):
            raise ParseError('expected )')
        self.p += 1
        c1, c2 = sorted((a[0], b[0]))
        r1, r2 = sorted((a[1], b[1]))
        vals = [self.value((c, r)) for c in range(c1, c2 + 1) for r in range(r1, r2 + 1)]
        err = combine_err(vals)
        if err:
            return err
        if name == 'SUM':
            return sum(vals)
        if name == 'MIN':
            return min(vals)
        return max(vals)


def col_name(i):
    return chr(ord('A') + i)


def main():
    cells = {}
    refs_seen = []  # (col,row) mentioned anywhere
    for raw in sys.stdin:
        line = raw.strip()
        if not line or '=' not in line:
            continue
        left, right = line.split('=', 1)
        name = left.strip()
        m = re.match(r'^([A-Z])(\d+)$', name)
        if not m:
            continue
        key = (ord(m.group(1)) - ord('A'), int(m.group(2)))
        content = right.strip()
        if content.startswith('='):
            try:
                toks = tokenize(content[1:])
            except ParseError:
                toks = []
            cells[key] = ('formula', toks)
            for kind, val in toks:
                if kind == 'ref':
                    refs_seen.append(split_ref(val))
        else:
            try:
                cells[key] = ('int', int(content))
            except ValueError:
                cells[key] = ('int', 0)
        refs_seen.append(key)

    if not cells:
        return

    # Also account for cells mentioned in formula ranges (they are in refs_seen).
    all_keys = list(refs_seen)
    max_col = max(c for c, _ in all_keys)
    max_row = max(r for _, r in all_keys)

    sheet = Sheet(cells)
    lines = ['\t' + '\t'.join(col_name(c) for c in range(max_col + 1))]
    for r in range(1, max_row + 1):
        row_vals = []
        for c in range(max_col + 1):
            key = (c, r)
            if key in cells:
                v = sheet.value(key)
                if v == CYCLE:
                    row_vals.append('#CYCLE')
                elif v == DIV0:
                    row_vals.append('#DIV0')
                else:
                    row_vals.append(str(v))
            else:
                row_vals.append('')
        lines.append(str(r) + '\t' + '\t'.join(row_vals))
    sys.stdout.write('\n'.join(lines) + '\n')


if __name__ == '__main__':
    main()
