import sys, re

sys.setrecursionlimit(100000)
cells = {}
maxc = 0
maxr = 0


def note(ref):
    global maxc, maxr
    c = ord(ref[0]) - 64
    r = int(ref[1:])
    maxc = max(maxc, c)
    maxr = max(maxr, r)


for line in sys.stdin.read().split("\n"):
    if "=" not in line:
        continue
    k, v = line.split("=", 1)
    k = k.strip()
    v = v.strip()
    if not re.fullmatch(r'[A-Z]\d{1,2}', k):
        continue
    note(k)
    cells[k] = v
    if v.startswith("="):
        for m in re.finditer(r'[A-Z]\d{1,2}', v):
            note(m.group(0))

memo = {}
visiting = set()
TOK = re.compile(r'\s*(\d+|[A-Z]\d{1,2}:[A-Z]\d{1,2}|[A-Z]+\d{1,2}|[A-Z]+|[-+*/():])')


def comb(*errs):
    if "CYCLE" in errs:
        return "CYCLE"
    if "DIV0" in errs:
        return "DIV0"
    return None


def cell(ref):
    if ref in memo:
        return memo[ref]
    if ref in visiting:
        return (0, "CYCLE")
    v = cells.get(ref)
    if v is None or v == "":
        return (0, None)
    if not v.startswith("="):
        try:
            r = (int(v), None)
        except ValueError:
            r = (0, None)
        memo[ref] = r
        return r
    visiting.add(ref)
    try:
        r = evalf(v[1:])
    except RecursionError:
        r = (0, "CYCLE")
    visiting.discard(ref)
    memo[ref] = r
    return r


def tdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


def evalf(s):
    toks = []
    pos = 0
    s = s.rstrip()
    while pos < len(s):
        m = TOK.match(s, pos)
        if not m:
            return (0, None)
        toks.append(m.group(1))
        pos = m.end()
    p = [0]

    def peek():
        return toks[p[0]] if p[0] < len(toks) else None

    def nxt():
        t = peek()
        p[0] += 1
        return t

    def expr():
        v, e = term()
        while peek() in ("+", "-"):
            op = nxt()
            v2, e2 = term()
            v = v + v2 if op == "+" else v - v2
            e = comb(e, e2)
        return v, e

    def term():
        v, e = unary()
        while peek() in ("*", "/"):
            op = nxt()
            v2, e2 = unary()
            ee = comb(e, e2)
            if op == "*":
                v = v * v2
            else:
                if v2 == 0:
                    ee = comb(ee, "DIV0")
                    v = 0
                else:
                    v = tdiv(v, v2)
            e = ee
        return v, e

    def unary():
        if peek() == "-":
            nxt()
            v, e = unary()
            return -v, e
        if peek() == "+":
            nxt()
            return unary()
        return atom()

    def atom():
        t = nxt()
        if t is None:
            return 0, None
        if t == "(":
            v, e = expr()
            if peek() == ")":
                nxt()
            return v, e
        if t.isdigit():
            return int(t), None
        if ":" in t:
            return 0, None
        if re.fullmatch(r'[A-Z]\d{1,2}', t):
            return cell(t)
        if re.fullmatch(r'[A-Z]+', t):
            fn = t
            if peek() == "(":
                nxt()
                a = nxt()
                if peek() == ")":
                    nxt()
                m = re.fullmatch(r'([A-Z])(\d{1,2}):([A-Z])(\d{1,2})', a or "")
                if not m:
                    return 0, None
                c1, r1, c2, r2 = m.group(1), int(m.group(2)), m.group(3), int(m.group(4))
                ca, cb = sorted((ord(c1), ord(c2)))
                ra, rb = sorted((r1, r2))
                vals = []
                errs = []
                for c in range(ca, cb + 1):
                    for r in range(ra, rb + 1):
                        v, e = cell(chr(c) + str(r))
                        vals.append(v)
                        errs.append(e)
                e = comb(*errs)
                if fn == "SUM":
                    return sum(vals), e
                if fn == "MIN":
                    return min(vals), e
                if fn == "MAX":
                    return max(vals), e
                return 0, e
        return 0, None

    return expr()


lines = ["\t" + "\t".join(chr(65 + i) for i in range(maxc))]
for r in range(1, maxr + 1):
    row = [str(r)]
    for c in range(maxc):
        ref = chr(65 + c) + str(r)
        if ref not in cells or cells[ref] == "":
            row.append("")
            continue
        v, e = cell(ref)
        row.append("#" + e if e else str(v))
    lines.append("\t".join(row))
print("\n".join(lines))
