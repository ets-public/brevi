import sys
import re

TOKRE = re.compile(
    r"(?P<str>'(?:[^']|'')*')"
    r"|(?P<num>-?[0-9]+)"
    r"|(?P<name>[A-Za-z][A-Za-z0-9_]*)"
    r"|(?P<op><=|>=|!=|=|<|>)"
    r"|(?P<punc>[(),*])"
)


class Syn(Exception):
    pass


class Err(Exception):
    pass


def tokenize(s):
    toks = []
    i = 0
    n = len(s)
    while i < n:
        if s[i] in ' \t':
            i += 1
            continue
        m = TOKRE.match(s, i)
        if not m:
            raise Syn()
        toks.append((m.lastgroup, m.group()))
        i = m.end()
    return toks


class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else (None, None)

    def peek2(self):
        return self.t[self.i + 1] if self.i + 1 < len(self.t) else (None, None)

    def take(self):
        tok = self.peek()
        if tok[0] is None:
            raise Syn()
        self.i += 1
        return tok

    def kw(self, w):
        if self.take() != ('name', w):
            raise Syn()

    def is_kw(self, w):
        return self.peek() == ('name', w)

    def name(self):
        k, v = self.take()
        if k != 'name':
            raise Syn()
        return v

    def punc(self, c):
        if self.take() != ('punc', c):
            raise Syn()

    def is_punc(self, c):
        return self.peek() == ('punc', c)

    def value(self):
        k, v = self.take()
        if k == 'num':
            return ('int', int(v))
        if k == 'str':
            return ('text', v[1:-1].replace("''", "'"))
        raise Syn()

    def op(self):
        k, v = self.take()
        if k != 'op':
            raise Syn()
        return v

    def done(self):
        if self.i != len(self.t):
            raise Syn()

    def where(self):
        conds = []
        if not self.is_kw('WHERE'):
            return conds
        self.i += 1
        while True:
            c = self.name()
            o = self.op()
            v = self.value()
            conds.append((c, o, v))
            if self.is_kw('AND'):
                self.i += 1
                continue
            break
        return conds


def parse(line):
    p = Parser(tokenize(line))
    w = p.name()
    if w == 'CREATE':
        p.kw('TABLE')
        t = p.name()
        p.punc('(')
        cols = []
        while True:
            cn = p.name()
            ty = p.name()
            if ty not in ('INT', 'TEXT'):
                raise Syn()
            cols.append((cn, ty))
            if p.is_punc(','):
                p.i += 1
                continue
            break
        p.punc(')')
        p.done()
        return ('create', t, cols)
    if w == 'INSERT':
        p.kw('INTO')
        t = p.name()
        p.kw('VALUES')
        p.punc('(')
        vals = []
        while True:
            vals.append(p.value())
            if p.is_punc(','):
                p.i += 1
                continue
            break
        p.punc(')')
        p.done()
        return ('insert', t, vals)
    if w == 'SELECT':
        if p.peek() == ('name', 'COUNT') and p.peek2() == ('punc', '('):
            p.i += 2
            p.punc('*')
            p.punc(')')
            p.kw('FROM')
            t = p.name()
            conds = p.where()
            p.done()
            return ('count', t, conds)
        if p.is_punc('*'):
            p.i += 1
            cols = None
        else:
            cols = [p.name()]
            while p.is_punc(','):
                p.i += 1
                cols.append(p.name())
        p.kw('FROM')
        t = p.name()
        conds = p.where()
        order = None
        if p.is_kw('ORDER'):
            p.i += 1
            p.kw('BY')
            oc = p.name()
            desc = False
            if p.is_kw('DESC'):
                p.i += 1
                desc = True
            order = (oc, desc)
        limit = None
        if p.is_kw('LIMIT'):
            p.i += 1
            k, v = p.take()
            if k != 'num' or int(v) < 0:
                raise Syn()
            limit = int(v)
        p.done()
        return ('select', t, cols, conds, order, limit)
    if w == 'UPDATE':
        t = p.name()
        p.kw('SET')
        c = p.name()
        if p.op() != '=':
            raise Syn()
        v = p.value()
        conds = p.where()
        p.done()
        return ('update', t, c, v, conds)
    if w == 'DELETE':
        p.kw('FROM')
        t = p.name()
        conds = p.where()
        p.done()
        return ('delete', t, conds)
    raise Syn()


OPS = {
    '=': lambda a, b: a == b,
    '!=': lambda a, b: a != b,
    '<': lambda a, b: a < b,
    '>': lambda a, b: a > b,
    '<=': lambda a, b: a <= b,
    '>=': lambda a, b: a >= b,
}


def get_table(db, t):
    if t not in db:
        raise Err("error: no table %s" % t)
    return db[t]


def check_cols(tbl, names):
    known = [c for c, _ in tbl['cols']]
    for n in names:
        if n not in known:
            raise Err("error: no column %s" % n)


def check_types(tbl, pairs):
    types = dict(tbl['cols'])
    for c, v in pairs:
        if (types[c] == 'INT') != (v[0] == 'int'):
            raise Err("error: type mismatch")


def match_row(idx, row, conds):
    for c, o, v in conds:
        if not OPS[o](row[idx[c]], v[1]):
            return False
    return True


def execute(db, st):
    kind = st[0]
    if kind == 'create':
        _, t, cols = st
        if t in db:
            return ["error: table %s exists" % t]
        db[t] = {'cols': cols, 'rows': []}
        return ['ok']

    tbl = get_table(db, st[1])
    names = [c for c, _ in tbl['cols']]
    idx = {c: i for i, c in enumerate(names)}

    if kind == 'insert':
        vals = st[2]
        if len(vals) != len(names):
            raise Err("error: type mismatch")
        check_types(tbl, list(zip(names, vals)))
        tbl['rows'].append([v[1] for v in vals])
        return ['ok']

    if kind == 'select':
        _, t, cols, conds, order, limit = st
        if cols is None:
            cols = names
        check_cols(tbl, cols)
        check_cols(tbl, [c for c, _, _ in conds])
        if order:
            check_cols(tbl, [order[0]])
        check_types(tbl, [(c, v) for c, _, v in conds])
        rows = [r for r in tbl['rows'] if match_row(idx, r, conds)]
        if order:
            oc, desc = order
            rows = sorted(rows, key=lambda r: r[idx[oc]], reverse=desc)
        if limit is not None:
            rows = rows[:limit]
        ci = [idx[c] for c in cols]
        out = [' | '.join(cols)]
        for r in rows:
            out.append(' | '.join(str(r[i]) for i in ci))
        out.append('(%d rows)' % len(rows))
        return out

    if kind == 'count':
        conds = st[2]
        check_cols(tbl, [c for c, _, _ in conds])
        check_types(tbl, [(c, v) for c, _, v in conds])
        n = sum(1 for r in tbl['rows'] if match_row(idx, r, conds))
        return [str(n)]

    if kind == 'update':
        _, t, c, v, conds = st
        check_cols(tbl, [c])
        check_cols(tbl, [cc for cc, _, _ in conds])
        check_types(tbl, [(c, v)] + [(cc, vv) for cc, _, vv in conds])
        n = 0
        for r in tbl['rows']:
            if match_row(idx, r, conds):
                r[idx[c]] = v[1]
                n += 1
        return ['updated %d' % n]

    if kind == 'delete':
        conds = st[2]
        check_cols(tbl, [c for c, _, _ in conds])
        check_types(tbl, [(c, v) for c, _, v in conds])
        before = len(tbl['rows'])
        tbl['rows'] = [r for r in tbl['rows'] if not match_row(idx, r, conds)]
        return ['deleted %d' % (before - len(tbl['rows']))]

    raise Syn()


def main():
    db = {}
    out = []
    for raw in sys.stdin.read().splitlines():
        line = raw.strip()
        if not line:
            continue
        try:
            st = parse(line)
            out.extend(execute(db, st))
        except Syn:
            out.append("error: syntax")
        except Err as e:
            out.append(str(e))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == '__main__':
    main()
