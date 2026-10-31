import sys, re

N = r"[A-Za-z][A-Za-z0-9_]*"
V = r"(?:-?[0-9]+|'(?:[^']|'')*')"
OP = r"(?:!=|<=|>=|=|<|>)"
C = r"%s %s %s" % (N, OP, V)
COND = r"%s(?: AND %s)*" % (C, C)
WH = r"(?: WHERE (%s))?" % COND

R_CREATE = re.compile(r"^CREATE TABLE (%s) \(((?:%s (?:INT|TEXT))(?:, %s (?:INT|TEXT))*)\)$" % (N, N, N))
R_INSERT = re.compile(r"^INSERT INTO (%s) VALUES \((%s(?:, %s)*)\)$" % (N, V, V))
R_SELECT = re.compile(r"^SELECT (\*|%s(?:, %s)*) FROM (%s)%s(?: ORDER BY (%s)( DESC)?)?(?: LIMIT ([0-9]+))?$" % (N, N, N, WH, N))
R_COUNT = re.compile(r"^SELECT COUNT\(\*\) FROM (%s)%s$" % (N, WH))
R_UPDATE = re.compile(r"^UPDATE (%s) SET (%s) = (%s)%s$" % (N, N, V, WH))
R_DELETE = re.compile(r"^DELETE FROM (%s)%s$" % (N, WH))
R_C = re.compile(r"(%s) (%s) (%s)" % (N, OP, V))
R_V = re.compile(V)

tables = {}

def parse_v(s):
    if s[0] == "'":
        return s[1:-1].replace("''", "'")
    return int(s)

def vtype(s):
    return 'TEXT' if s[0] == "'" else 'INT'

def fmtv(x):
    return str(x)

class Err(Exception):
    pass

def conds(w):
    return R_C.findall(w) if w else []

def check_conds(cs, cols):
    for c, op, v in cs:
        if c not in cols:
            raise Err("error: no column " + c)

def check_ctypes(cs, cols):
    for c, op, v in cs:
        if vtype(v) != cols[c][1]:
            raise Err("error: type mismatch")

def matcher(cs, cols):
    ops = {
        '=': lambda a, b: a == b, '!=': lambda a, b: a != b,
        '<': lambda a, b: a < b, '>': lambda a, b: a > b,
        '<=': lambda a, b: a <= b, '>=': lambda a, b: a >= b,
    }
    items = [(cols[c][0], ops[op], parse_v(v)) for c, op, v in cs]
    return lambda row: all(f(row[i], v) for i, f, v in items)

def get_table(name):
    if name not in tables:
        raise Err("error: no table " + name)
    t = tables[name]
    cols = {c: (i, ty) for i, (c, ty) in enumerate(t['cols'])}
    return t, cols

def run(line, out):
    m = R_CREATE.match(line)
    if m:
        name = m.group(1)
        if name in tables:
            raise Err("error: table %s exists" % name)
        cols = [tuple(x.split(' ')) for x in m.group(2).split(', ')]
        tables[name] = {'cols': cols, 'rows': []}
        out.append("ok")
        return
    m = R_INSERT.match(line)
    if m:
        t, cols = get_table(m.group(1))
        vals = R_V.findall(m.group(2))
        if len(vals) != len(t['cols']):
            raise Err("error: type mismatch")
        for v, (c, ty) in zip(vals, t['cols']):
            if vtype(v) != ty:
                raise Err("error: type mismatch")
        t['rows'].append([parse_v(v) for v in vals])
        out.append("ok")
        return
    m = R_COUNT.match(line)
    if m:
        t, cols = get_table(m.group(1))
        cs = conds(m.group(2))
        check_conds(cs, cols)
        check_ctypes(cs, cols)
        f = matcher(cs, cols)
        out.append(str(sum(1 for r in t['rows'] if f(r))))
        return
    m = R_SELECT.match(line)
    if m:
        sel, tn, w, ob, desc, lim = m.groups()
        t, cols = get_table(tn)
        if sel == '*':
            names = [c for c, _ in t['cols']]
        else:
            names = sel.split(', ')
            for c in names:
                if c not in cols:
                    raise Err("error: no column " + c)
        cs = conds(w)
        check_conds(cs, cols)
        if ob and ob not in cols:
            raise Err("error: no column " + ob)
        check_ctypes(cs, cols)
        f = matcher(cs, cols)
        rows = [r for r in t['rows'] if f(r)]
        if ob:
            i = cols[ob][0]
            rows.sort(key=lambda r: r[i], reverse=bool(desc))
        if lim is not None:
            rows = rows[:int(lim)]
        out.append(' | '.join(names))
        idx = [cols[c][0] for c in names]
        for r in rows:
            out.append(' | '.join(fmtv(r[i]) for i in idx))
        out.append("(%d rows)" % len(rows))
        return
    m = R_UPDATE.match(line)
    if m:
        tn, c, v, w = m.groups()
        t, cols = get_table(tn)
        if c not in cols:
            raise Err("error: no column " + c)
        cs = conds(w)
        check_conds(cs, cols)
        if vtype(v) != cols[c][1]:
            raise Err("error: type mismatch")
        check_ctypes(cs, cols)
        f = matcher(cs, cols)
        n = 0
        pv = parse_v(v)
        for r in t['rows']:
            if f(r):
                r[cols[c][0]] = pv
                n += 1
        out.append("updated %d" % n)
        return
    m = R_DELETE.match(line)
    if m:
        t, cols = get_table(m.group(1))
        cs = conds(m.group(2))
        check_conds(cs, cols)
        check_ctypes(cs, cols)
        f = matcher(cs, cols)
        keep = [r for r in t['rows'] if not f(r)]
        n = len(t['rows']) - len(keep)
        t['rows'] = keep
        out.append("deleted %d" % n)
        return
    raise Err("error: syntax")

def main():
    out = []
    for line in sys.stdin.read().split('\n'):
        if not line.strip():
            continue
        try:
            run(line, out)
        except Err as e:
            out.append(str(e))
    print('\n'.join(out))

main()
