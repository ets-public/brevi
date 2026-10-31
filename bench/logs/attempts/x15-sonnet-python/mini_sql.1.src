import sys, re

N = r"[A-Za-z][A-Za-z0-9_]*"
V = r"(?:-?\d+|'(?:[^']|'')*')"
C = N + r" (?:=|!=|<=|>=|<|>) " + V
COND = C + r"(?: AND " + C + r")*"
CRE = re.compile(r"^CREATE TABLE (%s) \((%s (?:INT|TEXT)(?:, %s (?:INT|TEXT))*)\)$" % (N, N, N))
INS = re.compile(r"^INSERT INTO (%s) VALUES \((%s(?:, %s)*)\)$" % (N, V, V))
SEL = re.compile(r"^SELECT (\*|%s(?:, %s)*) FROM (%s)(?: WHERE (%s))?(?: ORDER BY (%s)( DESC)?)?(?: LIMIT (\d+))?$" % (N, N, N, COND, N))
CNT = re.compile(r"^SELECT COUNT\(\*\) FROM (%s)(?: WHERE (%s))?$" % (N, COND))
UPD = re.compile(r"^UPDATE (%s) SET (%s) = (%s)(?: WHERE (%s))?$" % (N, N, V, COND))
DEL = re.compile(r"^DELETE FROM (%s)(?: WHERE (%s))?$" % (N, COND))
CP = re.compile("(%s) (=|!=|<=|>=|<|>) (%s)" % (N, V))
VP = re.compile(V)

tables = {}

def val(s):
    if s[0] == "'":
        return s[1:-1].replace("''", "'")
    return int(s)

def vals(s):
    return [val(m.group(0)) for m in VP.finditer(s)]

def conds(s):
    if s is None:
        return []
    return [(m.group(1), m.group(2), val(m.group(3))) for m in CP.finditer(s)]

class Err(Exception):
    pass

def getcols(t, names, cs):
    for n in names:
        if n not in t[0]:
            raise Err("error: no column " + n)

def check_conds(t, cs):
    for c, op, v in cs:
        if c not in t[0]:
            raise Err("error: no column " + c)
    for c, op, v in cs:
        if (t[0][c][1] == "INT") != isinstance(v, int):
            raise Err("error: type mismatch")

OPS = {"=": lambda a, b: a == b, "!=": lambda a, b: a != b, "<": lambda a, b: a < b,
       ">": lambda a, b: a > b, "<=": lambda a, b: a <= b, ">=": lambda a, b: a >= b}

def test(t, row, cs):
    return all(OPS[op](row[t[0][c][0]], v) for c, op, v in cs)

def gettable(name):
    if name not in tables:
        raise Err("error: no table " + name)
    return tables[name]

def fmt(x):
    return str(x)

def run(line):
    m = CRE.match(line)
    if m:
        if m.group(1) in tables:
            raise Err("error: table %s exists" % m.group(1))
        cols = {}
        order = []
        for d in m.group(2).split(", "):
            n, ty = d.split(" ")
            if n not in cols:
                cols[n] = (len(order), ty)
                order.append(n)
        tables[m.group(1)] = (cols, order, [])
        return ["ok"]
    m = INS.match(line)
    if m:
        t = gettable(m.group(1))
        vs = vals(m.group(2))
        if len(vs) != len(t[1]):
            raise Err("error: type mismatch")
        for n, v in zip(t[1], vs):
            if (t[0][n][1] == "INT") != isinstance(v, int):
                raise Err("error: type mismatch")
        t[2].append(vs)
        return ["ok"]
    m = CNT.match(line)
    if m:
        t = gettable(m.group(1))
        cs = conds(m.group(2))
        check_conds(t, cs)
        return [str(sum(1 for r in t[2] if test(t, r, cs)))]
    m = SEL.match(line)
    if m:
        t = gettable(m.group(2))
        cols = t[1] if m.group(1) == "*" else m.group(1).split(", ")
        cs = conds(m.group(3))
        getcols(t, cols, cs)
        for c, op, v in cs:
            if c not in t[0]:
                raise Err("error: no column " + c)
        if m.group(4) and m.group(4) not in t[0]:
            raise Err("error: no column " + m.group(4))
        check_conds(t, cs)
        rows = [r for r in t[2] if test(t, r, cs)]
        if m.group(4):
            i = t[0][m.group(4)][0]
            rows = sorted(rows, key=lambda r: r[i], reverse=bool(m.group(5)))
            if m.group(5):
                # stable descending: keep insertion order for equals
                rows = sorted([r for r in t[2] if test(t, r, cs)], key=lambda r: r[i], reverse=True)
        if m.group(6) is not None:
            rows = rows[:int(m.group(6))]
        idx = [t[0][c][0] for c in cols]
        out = [" | ".join(cols)]
        for r in rows:
            out.append(" | ".join(fmt(r[i]) for i in idx))
        out.append("(%d rows)" % len(rows))
        return out
    m = UPD.match(line)
    if m:
        t = gettable(m.group(1))
        col = m.group(2)
        v = val(m.group(3))
        cs = conds(m.group(4))
        if col not in t[0]:
            raise Err("error: no column " + col)
        for c, op, _ in cs:
            if c not in t[0]:
                raise Err("error: no column " + c)
        if (t[0][col][1] == "INT") != isinstance(v, int):
            raise Err("error: type mismatch")
        check_conds(t, cs)
        n = 0
        for r in t[2]:
            if test(t, r, cs):
                r[t[0][col][0]] = v
                n += 1
        return ["updated %d" % n]
    m = DEL.match(line)
    if m:
        t = gettable(m.group(1))
        cs = conds(m.group(2))
        check_conds(t, cs)
        keep = [r for r in t[2] if not test(t, r, cs)]
        n = len(t[2]) - len(keep)
        t[2][:] = keep
        return ["deleted %d" % n]
    return ["error: syntax"]

def main():
    out = []
    for line in sys.stdin.read().split("\n"):
        if not line.strip():
            continue
        try:
            out.extend(run(line))
        except Err as e:
            out.append(str(e))
    print("\n".join(out))

main()
