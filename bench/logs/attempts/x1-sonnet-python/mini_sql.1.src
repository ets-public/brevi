import sys, re

NAME = r'[A-Za-z][A-Za-z0-9_]*'
VAL = r"(?:-?\d+|'(?:[^']|'')*')"
tables = {}

class Syn(Exception):
    pass

class P:
    def __init__(self, s):
        self.s = s
        self.i = 0
    def m(self, pat):
        r = re.compile(pat).match(self.s, self.i)
        if r:
            self.i = r.end()
        return r
    def need(self, pat):
        r = self.m(pat)
        if not r:
            raise Syn()
        return r
    def end(self):
        if self.i != len(self.s):
            raise Syn()

def val(t):
    if t[0] == "'":
        return t[1:-1].replace("''", "'")
    return int(t)

def parse_cond(p):
    conds = []
    while True:
        r = p.need('(' + NAME + r') (=|!=|<=|>=|<|>) (' + VAL + ')')
        conds.append((r.group(1), r.group(2), val(r.group(3))))
        if not p.m(' AND '):
            break
    return conds

def parse_vals(p):
    p.need(r'\(')
    vs = [val(p.need('(' + VAL + ')').group(1))]
    while p.m(', '):
        vs.append(val(p.need('(' + VAL + ')').group(1)))
    p.need(r'\)')
    return vs

def ok_type(t, v):
    return isinstance(v, int) if t == 'INT' else isinstance(v, str)

def check_cols(tab, names):
    for n in names:
        if n not in tab['idx']:
            return "error: no column " + n
    return None

def check_conds(tab, conds):
    e = check_cols(tab, [c[0] for c in conds])
    if e:
        return e
    for c, op, v in conds:
        if not ok_type(tab['cols'][tab['idx'][c]][1], v):
            return "error: type mismatch"
    return None

OPS = {'=': lambda a, b: a == b, '!=': lambda a, b: a != b, '<': lambda a, b: a < b,
       '>': lambda a, b: a > b, '<=': lambda a, b: a <= b, '>=': lambda a, b: a >= b}

def match(tab, row, conds):
    for c, op, v in conds:
        if not OPS[op](row[tab['idx'][c]], v):
            return False
    return True

def show(v):
    return str(v)

def run(line):
    p = P(line)
    if p.m('CREATE TABLE '):
        t = p.need('(' + NAME + ') \\(').group(1)
        cols = []
        while True:
            r = p.need('(' + NAME + ') (INT|TEXT)')
            cols.append((r.group(1), r.group(2)))
            if not p.m(', '):
                break
        p.need(r'\)')
        p.end()
        if t in tables:
            return "error: table %s exists" % t
        tables[t] = {'cols': cols, 'idx': {c[0]: i for i, c in enumerate(cols)}, 'rows': []}
        return "ok"
    if p.m('INSERT INTO '):
        t = p.need('(' + NAME + ') VALUES ').group(1)
        vs = parse_vals(p)
        p.end()
        if t not in tables:
            return "error: no table " + t
        tab = tables[t]
        if len(vs) != len(tab['cols']):
            return "error: type mismatch"
        for (cn, ct), v in zip(tab['cols'], vs):
            if not ok_type(ct, v):
                return "error: type mismatch"
        tab['rows'].append(vs)
        return "ok"
    if p.m('SELECT '):
        cnt = False
        names = None
        if p.m(r'COUNT\(\*\)'):
            cnt = True
        elif p.m(r'\*'):
            pass
        else:
            names = [p.need('(' + NAME + ')').group(1)]
            while p.m(', '):
                names.append(p.need('(' + NAME + ')').group(1))
        t = p.need(' FROM (' + NAME + ')').group(1)
        conds = []
        if p.m(' WHERE '):
            conds = parse_cond(p)
        order = None
        desc = False
        limit = None
        if not cnt:
            r = p.m(' ORDER BY (' + NAME + ')')
            if r:
                order = r.group(1)
                if p.m(' DESC'):
                    desc = True
            r = p.m(r' LIMIT (\d+)')
            if r:
                limit = int(r.group(1))
        p.end()
        if t not in tables:
            return "error: no table " + t
        tab = tables[t]
        chk = (names or []) + [c[0] for c in conds] + ([order] if order else [])
        e = check_cols(tab, chk)
        if e:
            return e
        e = check_conds(tab, conds)
        if e:
            return e
        rows = [r for r in tab['rows'] if match(tab, r, conds)]
        if cnt:
            return str(len(rows))
        if order:
            k = tab['idx'][order]
            rows = sorted(rows, key=lambda r: r[k], reverse=desc)
        if limit is not None:
            rows = rows[:limit]
        if names is None:
            names = [c[0] for c in tab['cols']]
        ids = [tab['idx'][n] for n in names]
        out = [" | ".join(names)]
        for r in rows:
            out.append(" | ".join(show(r[i]) for i in ids))
        out.append("(%d rows)" % len(rows))
        return "\n".join(out)
    if p.m('UPDATE '):
        t = p.need('(' + NAME + ') SET (' + NAME + ') = (' + VAL + ')')
        tn, col, v = t.group(1), t.group(2), val(t.group(3))
        conds = []
        if p.m(' WHERE '):
            conds = parse_cond(p)
        p.end()
        if tn not in tables:
            return "error: no table " + tn
        tab = tables[tn]
        e = check_cols(tab, [col] + [c[0] for c in conds])
        if e:
            return e
        if not ok_type(tab['cols'][tab['idx'][col]][1], v):
            return "error: type mismatch"
        e = check_conds(tab, conds)
        if e:
            return e
        n = 0
        k = tab['idx'][col]
        for r in tab['rows']:
            if match(tab, r, conds):
                r[k] = v
                n += 1
        return "updated %d" % n
    if p.m('DELETE FROM '):
        tn = p.need('(' + NAME + ')').group(1)
        conds = []
        if p.m(' WHERE '):
            conds = parse_cond(p)
        p.end()
        if tn not in tables:
            return "error: no table " + tn
        tab = tables[tn]
        e = check_conds(tab, conds)
        if e:
            return e
        keep = [r for r in tab['rows'] if not match(tab, r, conds)]
        n = len(tab['rows']) - len(keep)
        tab['rows'] = keep
        return "deleted %d" % n
    raise Syn()

out = []
for line in sys.stdin.read().split('\n'):
    if not line.strip():
        continue
    try:
        out.append(run(line))
    except Syn:
        out.append("error: syntax")
print("\n".join(out))
