import sys, re
cells = {}
maxc = 0; maxr = 0
def note(c, r):
    global maxc, maxr
    maxc = max(maxc, c); maxr = max(maxr, r)
for line in sys.stdin.read().split("\n"):
    if "=" not in line:
        continue
    k, v = line.split("=", 1)
    k = k.strip().upper(); v = v.strip()
    m = re.fullmatch(r"([A-Z])(\d+)", k)
    if not m:
        continue
    note(ord(m.group(1)) - 64, int(m.group(2)))
    cells[k] = v
    if v.startswith("="):
        for a, b in re.findall(r"([A-Za-z])(\d+)", v):
            note(ord(a.upper()) - 64, int(b))
TOK = re.compile(r"\s*(\d+|[A-Za-z]+\d*|[-+*/():])")
def tokenize(s):
    s = s.strip(); pos = 0; out = []
    while pos < len(s):
        m = TOK.match(s, pos)
        if not m:
            break
        out.append(m.group(1)); pos = m.end()
        while pos < len(s) and s[pos].isspace():
            pos += 1
    return out
memo = {}
stack = set()
def comb(*es):
    if "C" in es: return "C"
    if "D" in es: return "D"
    return None
def tdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q
def value(ref):
    if ref in memo: return memo[ref]
    if ref in stack: return "C"
    if ref not in cells: return 0
    stack.add(ref)
    c = cells[ref]
    if c.startswith("="):
        toks = tokenize(c[1:])
        try:
            val, p = expr(toks, 0)
        except Exception:
            val = 0
    else:
        try: val = int(c)
        except ValueError: val = 0
    stack.discard(ref)
    memo[ref] = val
    return val
def rng(a, b):
    c1, r1 = ord(a[0].upper()) - 64, int(a[1:]); c2, r2 = ord(b[0].upper()) - 64, int(b[1:])
    vs = []
    for c in range(min(c1, c2), max(c1, c2) + 1):
        for r in range(min(r1, r2), max(r1, r2) + 1):
            vs.append(value(chr(64 + c) + str(r)))
    return vs
def expr(t, i):
    v, i = term(t, i)
    while i < len(t) and t[i] in "+-":
        op = t[i]; w, i = term(t, i + 1)
        e = comb(v, w)
        v = e if e else (v + w if op == "+" else v - w)
    return v, i
def term(t, i):
    v, i = unary(t, i)
    while i < len(t) and t[i] in "*/":
        op = t[i]; w, i = unary(t, i + 1)
        e = comb(v, w)
        if e: v = e
        elif op == "*": v = v * w
        elif w == 0: v = "D"
        else: v = tdiv(v, w)
    return v, i
def unary(t, i):
    if t[i] == "-":
        v, i = unary(t, i + 1)
        return (v if isinstance(v, str) else -v), i
    if t[i] == "+":
        return unary(t, i + 1)
    return atom(t, i)
def atom(t, i):
    x = t[i]
    if x == "(":
        v, i = expr(t, i + 1)
        return v, i + 1
    if x.isdigit():
        return int(x), i + 1
    name = x.upper()
    if i + 1 < len(t) and t[i + 1] == "(":
        a = t[i + 2]
        if t[i + 3] == ":":
            b = t[i + 4]; i += 6
        else:
            b = a; i += 4
        vs = rng(a, b)
        e = comb(*vs)
        if e: return e, i
        if name == "SUM": return sum(vs), i
        if name == "MIN": return min(vs), i
        return max(vs), i
    return value(name), i + 1
cols = [chr(64 + c) for c in range(1, maxc + 1)]
if maxc and maxr:
    print("\t" + "\t".join(cols))
    for r in range(1, maxr + 1):
        row = []
        for c in cols:
            k = c + str(r)
            if k not in cells: row.append("")
            else:
                v = value(k)
                row.append("#CYCLE" if v == "C" else "#DIV0" if v == "D" else str(v))
        print(str(r) + "\t" + "\t".join(row))
