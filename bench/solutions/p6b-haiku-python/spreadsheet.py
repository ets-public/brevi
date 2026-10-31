import sys
import re

TOKEN = re.compile(r'\d+|[A-Z]+\d*|[-+*/():]')

REF = re.compile(r'[A-Z]\d+')
INT = re.compile(r'-?\d+')

cells = {}   # 'A1' -> content string
memo = {}
visiting = set()


def get(key):
    if key not in cells:
        return 0, None
    if key in memo:
        return memo[key]
    if key in visiting:
        return 0, 'CYCLE'
    visiting.add(key)
    content = cells[key]
    if INT.fullmatch(content):
        res = (int(content), None)
    elif content.startswith('='):
        res = evalf(content[1:])
    else:
        res = (0, None)
    visiting.discard(key)
    memo[key] = res
    return res


def evalf(text):
    s = re.sub(r'\s', '', text)
    toks = TOKEN.findall(s)
    pos = [0]
    errs = set()

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
            r = term()
            v = v + r if op == '+' else v - r
        return v

    def term():
        v = unary()
        while peek() in ('*', '/'):
            op = take()
            r = unary()
            if op == '*':
                v = v * r
            elif r == 0:
                errs.add('DIV0')
                v = 0
            else:
                qv = abs(v) // abs(r)
                v = qv if (v < 0) == (r < 0) else -qv
        return v

    def unary():
        if peek() == '-':
            take()
            return -unary()
        return primary()

    def cell_val(key):
        val, err = get(key)
        if err:
            errs.add(err)
        return val

    def primary():
        t = take()
        if t.isdigit():
            return int(t)
        if t == '(':
            v = expr()
            if peek() == ')':
                take()
            return v
        if REF.fullmatch(t):
            return cell_val(t)
        if t in ('SUM', 'MIN', 'MAX'):
            take()            # '('
            a = take()
            take()            # ':'
            b = take()
            take()            # ')'
            ca, ra = a[0], int(a[1:])
            cb, rb = b[0], int(b[1:])
            vals = []
            for c in range(min(ord(ca), ord(cb)), max(ord(ca), ord(cb)) + 1):
                for r in range(min(ra, rb), max(ra, rb) + 1):
                    vals.append(cell_val(f"{chr(c)}{r}"))
            if t == 'SUM':
                return sum(vals)
            if t == 'MIN':
                return min(vals)
            return max(vals)
        return 0

    v = expr()
    if 'CYCLE' in errs:
        return 0, 'CYCLE'
    if 'DIV0' in errs:
        return 0, 'DIV0'
    return v, None


def main():
    mentioned = []
    for line in sys.stdin.read().splitlines():
        if '=' not in line:
            continue
        left, right = line.split('=', 1)
        name = left.strip()
        cells[name] = right.strip()

    defined = list(cells.keys())
    for key, content in cells.items():
        if content.startswith('='):
            s = re.sub(r'\s', '', content[1:])
            for tok in TOKEN.findall(s):
                if REF.fullmatch(tok):
                    mentioned.append(tok)

    all_refs = [(k[0], int(k[1:])) for k in defined] + [(t[0], int(t[1:])) for t in mentioned]
    if all_refs:
        ncols = max(ord(c) - 64 for c, _ in all_refs)
        nrows = max(r for _, r in all_refs)
    else:
        ncols = 0
        nrows = 0
    letters = [chr(65 + i) for i in range(ncols)]

    lines = ['\t' + '\t'.join(letters)]
    for r in range(1, nrows + 1):
        row = [str(r)]
        for L in letters:
            key = f"{L}{r}"
            if key in cells:
                val, err = get(key)
                if err == 'CYCLE':
                    row.append('#CYCLE')
                elif err == 'DIV0':
                    row.append('#DIV0')
                else:
                    row.append(str(val))
            else:
                row.append('')
        lines.append('\t'.join(row))
    print('\n'.join(lines))


main()
