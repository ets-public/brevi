import sys, re

N = r"[A-Za-z][A-Za-z0-9_]*"
V = r"(?:-?\d+|'(?:[^']|'')*')"
CMP = N + r" (?:=|!=|<=|>=|<|>) " + V
COND = CMP + r"(?: AND " + CMP + r")*"
CMPG = re.compile("(" + N + ") (=|!=|<=|>=|<|>) (" + V + ")")

R_CREATE = re.compile(r"CREATE TABLE (%s) \(((?:%s (?:INT|TEXT))(?:, %s (?:INT|TEXT))*)\)" % (N, N, N))
R_INSERT = re.compile(r"INSERT INTO (%s) VALUES \((%s(?:, %s)*)\)" % (N, V, V))
R_SELECT = re.compile(r"SELECT (\*|COUNT\(\*\)|%s(?:, %s)*) FROM (%s)(?: WHERE (%s))?(?: ORDER BY (%s)( DESC)?)?(?: LIMIT (\d+))?" % (N, N, N, COND, N))
R_UPDATE = re.compile(r"UPDATE (%s) SET (%s) = (%s)(?: WHERE (%s))?" % (N, N, V, COND))
R_DELETE = re.compile(r"DELETE FROM (%s)(?: WHERE (%s))?" % (N, COND))

tables = {}

def parse_val(s):
    if s[0] == "'":
        return s[1:-1].replace("''", "'")
    return int(s)

def vtype(s):
    return "TEXT" if s[0] == "'" else "INT"

def parse_cond(c):
    if c is None:
        return []
    return [(m.group(1), m.group(2), m.group(3)) for m in
            (CMPG.fullmatch(p) for p in c.split(" AND "))] if " AND " in c and False else parse_cond2(c)

def parse_cond2(c):
    res = []
    pos = 0
    pat = re.compile("(" + N + ") (=|!=|<=|>=|<|>) (" + V + ")")
    while True:
        m = pat.match(c, pos)
        res.append((m.group(1), m.group(2), m.group(3)))
        pos = m.end()
        if pos >= len(c):
            break
        pos += 5
    return res

OPS = {
    "=": lambda a, b: a == b, "!=": lambda a, b: a != b,
    "<": lambda a, b: a < b, ">": lambda a, b: a > b,
    "<=": lambda a, b: a <= b, ">=": lambda a, b: a >= b,
}

def check_cond(t, conds):
    cols = [c for c, _, _ in conds]
    for c in cols:
        if c not in t["cols"]:
            return "error: no column " + c
    for c, op, v in conds:
        if t["types"][c] != vtype(v):
            return "error: type mismatch"
    return None

def matcher(t, conds):
    idx = [(t["cols"].index(c), OPS[op], parse_val(v)) for c, op, v in conds]
    return lambda row: all(f(row[i], v) for i, f, v in idx)

def fmt(v):
    return str(v)

def run(line):
    m = R_CREATE.fullmatch(line)
    if m:
        name = m.group(1)
        if name in tables:
            return "error: table %s exists" % name
        cols, types = [], {}
        for part in m.group(2).split(", "):
            c, ty = part.split(" ")
            cols.append(c)
            types[c] = ty
        tables[name] = {"cols": cols, "types": types, "rows": []}
        return "ok"
    m = R_INSERT.fullmatch(line)
    if m:
        t = tables.get(m.group(1))
        if t is None:
            return "error: no table " + m.group(1)
        vals = re.findall(V, m.group(2))
        if len(vals) != len(t["cols"]):
            return "error: type mismatch"
        for c, v in zip(t["cols"], vals):
            if t["types"][c] != vtype(v):
                return "error: type mismatch"
        t["rows"].append([parse_val(v) for v in vals])
        return "ok"
    m = R_SELECT.fullmatch(line)
    if m:
        sel, tn, cond, order, desc, limit = m.groups()
        t = tables.get(tn)
        if t is None:
            return "error: no table " + tn
        if sel == "COUNT(*)":
            if order or limit:
                return "error: syntax"
            conds = parse_cond(cond)
            e = check_cond(t, conds)
            if e:
                return e
            f = matcher(t, conds)
            return str(sum(1 for r in t["rows"] if f(r)))
        cols = t["cols"] if sel == "*" else sel.split(", ")
        for c in cols:
            if c not in t["cols"]:
                return "error: no column " + c
        conds = parse_cond(cond)
        e = check_cond(t, conds)
        if e:
            return e
        if order and order not in t["cols"]:
            return "error: no column " + order
        f = matcher(t, conds)
        rows = [r for r in t["rows"] if f(r)]
        if order:
            oi = t["cols"].index(order)
            rows = sorted(rows, key=lambda r: r[oi], reverse=bool(desc))
        if limit is not None:
            rows = rows[:int(limit)]
        ids = [t["cols"].index(c) for c in cols]
        out = [" | ".join(cols)]
        for r in rows:
            out.append(" | ".join(fmt(r[i]) for i in ids))
        out.append("(%d rows)" % len(rows))
        return "\n".join(out)
    m = R_UPDATE.fullmatch(line)
    if m:
        tn, col, val, cond = m.groups()
        t = tables.get(tn)
        if t is None:
            return "error: no table " + tn
        if col not in t["cols"]:
            return "error: no column " + col
        conds = parse_cond(cond)
        for c, _, _ in conds:
            if c not in t["cols"]:
                return "error: no column " + c
        if t["types"][col] != vtype(val):
            return "error: type mismatch"
        e = check_cond(t, conds)
        if e:
            return e
        f = matcher(t, conds)
        ci = t["cols"].index(col)
        n = 0
        for r in t["rows"]:
            if f(r):
                r[ci] = parse_val(val)
                n += 1
        return "updated %d" % n
    m = R_DELETE.fullmatch(line)
    if m:
        tn, cond = m.groups()
        t = tables.get(tn)
        if t is None:
            return "error: no table " + tn
        conds = parse_cond(cond)
        e = check_cond(t, conds)
        if e:
            return e
        f = matcher(t, conds)
        keep = [r for r in t["rows"] if not f(r)]
        n = len(t["rows"]) - len(keep)
        t["rows"] = keep
        return "deleted %d" % n
    return "error: syntax"

def main():
    out = []
    for line in sys.stdin.read().split("\n"):
        if line.strip() == "":
            continue
        out.append(run(line))
    print("\n".join(out))

main()
