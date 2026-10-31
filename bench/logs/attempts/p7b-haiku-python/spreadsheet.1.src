import sys
import re

CYC = 'CYCLE'
DIV = 'DIV0'
TOK = re.compile(r'[A-Z]+\(|[A-Z][0-9]+|[0-9]+|[-+*/():]')

cells = {}
memo = {}
state = {}
mentioned = set()


def split_ref(ref):
    return ord(ref[0]) - 65, int(ref[1:])


def colname(c):
    return chr(65 + c)


def comb(vals):
    errs = [v for v in vals if isinstance(v, str)]
    if not errs:
        return None
    return CYC if CYC in errs else DIV


def binop(op, a, b):
    if isinstance(a, str) or isinstance(b, str):
        return comb([a, b])
    if op == '+':
        return a + b
    if op == '-':
        return a - b
    if op == '*':
        return a * b
    if b == 0:
        return DIV
    q = abs(a) // abs(b)
    return -q if (a < 0) != (b < 0) else q


def agg(fn, a, b):
    mentioned.add(a)
    mentioned.add(b)
    ca, ra = split_ref(a)
    cb, rb = split_ref(b)
    vals = []
    for c in range(min(ca, cb), max(ca, cb) + 1):
        for r in range(min(ra, rb), max(ra, rb) + 1):
            vals.append(cell_val(colname(c) + str(r)))
    e = comb(vals)
    if e:
        return e
    if fn == 'SUM':
        return sum(vals)
    if fn == 'MIN':
        return min(vals)
    return max(vals)


def evalf(src):
    toks = TOK.findall(src.replace(' ', '').replace('\t', ''))
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None

    def take():
        t = toks[pos[0]]
        pos[0] += 1
        return t

    def expr():
        v = term()
        while peek() in ('+', '-'):
            op = take()
            v = binop(op, v, term())
        return v

    def term():
        v = unary()
        while peek() in ('*', '/'):
            op = take()
            v = binop(op, v, unary())
        return v

    def unary():
        if peek() == '-':
            take()
            v = unary()
            return v if isinstance(v, str) else -v
        return primary()

    def primary():
        t = take()
        if t == '(':
            v = expr()
            if peek() == ')':
                take()
            return v
        if t.endswith('('):
            fn = t[:-1]
            a = take()
            b = a
            if peek() == ':':
                take()
                b = take()
            if peek() == ')':
                take()
            return agg(fn, a, b)
        if t.isdigit():
            return int(t)
        mentioned.add(t)
        return cell_val(t)

    return expr()


def cell_val(ref):
    if ref not in cells:
        return 0
    if ref in memo:
        return memo[ref]
    if state.get(ref) == 1:
        return CYC
    state[ref] = 1
    c = cells[ref]
    v = c if isinstance(c, int) else evalf(c)
    state[ref] = 2
    memo[ref] = v
    return v


def main():
    for raw in sys.stdin.read().splitlines():
        line = raw.strip()
        if not line:
            continue
        left, _, right = line.partition('=')
        ref = left.strip().replace(' ', '')
        content = right.strip()
        if content.startswith('='):
            cells[ref] = content[1:]
        else:
            cells[ref] = int(content)

    for ref in list(cells):
        cell_val(ref)

    refs = set(cells) | mentioned
    if not refs:
        print('\t')
        return
    maxc = max(split_ref(r)[0] for r in refs)
    maxr = max(split_ref(r)[1] for r in refs)

    lines = ['\t' + '\t'.join(colname(c) for c in range(maxc + 1))]
    for r in range(1, maxr + 1):
        row = [str(r)]
        for c in range(maxc + 1):
            ref = colname(c) + str(r)
            if ref not in cells:
                row.append('')
            else:
                v = cell_val(ref)
                row.append('#' + v if isinstance(v, str) else str(v))
        lines.append('\t'.join(row))
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
