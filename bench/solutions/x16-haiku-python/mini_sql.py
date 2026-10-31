import sys
import re


class SyntaxErr(Exception):
    pass


class SqlErr(Exception):
    def __init__(self, msg):
        Exception.__init__(self, msg)
        self.msg = msg


OPS = ('=', '!=', '<', '>', '<=', '>=')

TOK = re.compile(r"""('(?:[^']|'')*')|(-?[0-9]+)|([A-Za-z][A-Za-z0-9_]*)|(!=|<=|>=|[=<>(),*])""")


def tokenize(s):
    toks = []
    pos = 0
    n = len(s)
    while pos < n:
        if s[pos] == ' ':
            pos += 1
            continue
        m = TOK.match(s, pos)
        if not m:
            raise SyntaxErr()
        if m.group(1) is not None:
            toks.append(('str', m.group(1)[1:-1].replace("''", "'")))
        elif m.group(2) is not None:
            toks.append(('int', int(m.group(2))))
        elif m.group(3) is not None:
            toks.append(('name', m.group(3)))
        else:
            toks.append(('op', m.group(4)))
        pos = m.end()
    return toks


class P:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else None

    def kw(self, word):
        if self.peek() == ('name', word):
            self.i += 1
            return
        raise SyntaxErr()

    def op(self, o):
        if self.peek() == ('op', o):
            self.i += 1
            return
        raise SyntaxErr()

    def try_op(self, o):
        if self.peek() == ('op', o):
            self.i += 1
            return True
        return False

    def name(self):
        tk = self.peek()
        if tk is not None and tk[0] == 'name':
            self.i += 1
            return tk[1]
        raise SyntaxErr()

    def value(self):
        tk = self.peek()
        if tk is not None and tk[0] in ('int', 'str'):
            self.i += 1
            return tk
        raise SyntaxErr()

    def end(self):
        if self.i != len(self.t):
            raise SyntaxErr()


def parse_cmp(p):
    c = p.name()
    tk = p.peek()
    if tk is None or tk[0] != 'op' or tk[1] not in OPS:
        raise SyntaxErr()
    p.i += 1
    v = p.value()
    return (c, tk[1], v)


def parse_where(p):
    if p.peek() != ('name', 'WHERE'):
        return []
    p.kw('WHERE')
    conds = [parse_cmp(p)]
    while p.peek() == ('name', 'AND'):
        p.kw('AND')
        conds.append(parse_cmp(p))
    return conds


def parse_statement(toks):
    p = P(toks)
    first = p.peek()
    if first is None or first[0] != 'name':
        raise SyntaxErr()
    w = first[1]

    if w == 'CREATE':
        p.kw('CREATE')
        p.kw('TABLE')
        t = p.name()
        p.op('(')
        cols = []
        while True:
            c = p.name()
            ty = p.name()
            if ty not in ('INT', 'TEXT'):
                raise SyntaxErr()
            cols.append((c, ty))
            if p.try_op(','):
                continue
            break
        p.op(')')
        p.end()
        return ('create', t, cols)

    if w == 'INSERT':
        p.kw('INSERT')
        p.kw('INTO')
        t = p.name()
        p.kw('VALUES')
        p.op('(')
        vals = [p.value()]
        while p.try_op(','):
            vals.append(p.value())
        p.op(')')
        p.end()
        return ('insert', t, vals)

    if w == 'SELECT':
        p.kw('SELECT')
        if p.peek() == ('name', 'COUNT'):
            p.kw('COUNT')
            p.op('(')
            p.op('*')
            p.op(')')
            p.kw('FROM')
            t = p.name()
            where = parse_where(p)
            p.end()
            return ('count', t, where)
        if p.try_op('*'):
            cols = '*'
        else:
            cols = [p.name()]
            while p.try_op(','):
                cols.append(p.name())
        p.kw('FROM')
        t = p.name()
        where = parse_where(p)
        order = None
        desc = False
        if p.peek() == ('name', 'ORDER'):
            p.kw('ORDER')
            p.kw('BY')
            order = p.name()
            if p.peek() == ('name', 'DESC'):
                p.kw('DESC')
                desc = True
        limit = None
        if p.peek() == ('name', 'LIMIT'):
            p.kw('LIMIT')
            v = p.value()
            if v[0] != 'int' or v[1] < 0:
                raise SyntaxErr()
            limit = v[1]
        p.end()
        return ('select', t, cols, where, order, desc, limit)

    if w == 'UPDATE':
        p.kw('UPDATE')
        t = p.name()
        p.kw('SET')
        c = p.name()
        p.op('=')
        v = p.value()
        where = parse_where(p)
        p.end()
        return ('update', t, c, v, where)

    if w == 'DELETE':
        p.kw('DELETE')
        p.kw('FROM')
        t = p.name()
        where = parse_where(p)
        p.end()
        return ('delete', t, where)

    raise SyntaxErr()


class Table:
    def __init__(self, cols):
        self.cols = cols
        self.index = {}
        for i, (c, _) in enumerate(cols):
            self.index[c] = i
        self.rows = []


def get_table(db, t):
    if t not in db:
        raise SqlErr('error: no table %s' % t)
    return db[t]


def col_index(tbl, name):
    if name not in tbl.index:
        raise SqlErr('error: no column %s' % name)
    return tbl.index[name]


def check_type(ty, v):
    if (ty == 'INT') != (v[0] == 'int'):
        raise SqlErr('error: type mismatch')


def check_where(tbl, where):
    conds = []
    for c, op, v in where:
        ci = col_index(tbl, c)
        check_type(tbl.cols[ci][1], v)
        conds.append((ci, op, v))
    return conds


def cmpop(a, op, b):
    if op == '=':
        return a == b
    if op == '!=':
        return a != b
    if op == '<':
        return a < b
    if op == '>':
        return a > b
    if op == '<=':
        return a <= b
    return a >= b


def matches(row, conds):
    for ci, op, v in conds:
        if not cmpop(row[ci], op, v[1]):
            return False
    return True


def execute(line, db):
    st = parse_statement(tokenize(line))
    kind = st[0]

    if kind == 'create':
        _, t, cols = st
        if t in db:
            raise SqlErr('error: table %s exists' % t)
        db[t] = Table(cols)
        return ['ok']

    if kind == 'insert':
        _, t, vals = st
        tbl = get_table(db, t)
        if len(vals) != len(tbl.cols):
            raise SqlErr('error: type mismatch')
        for (cn, ty), v in zip(tbl.cols, vals):
            check_type(ty, v)
        tbl.rows.append([v[1] for v in vals])
        return ['ok']

    if kind == 'select':
        _, t, cols, where, order, desc, limit = st
        tbl = get_table(db, t)
        if cols == '*':
            sel = list(range(len(tbl.cols)))
        else:
            sel = [col_index(tbl, c) for c in cols]
        conds = check_where(tbl, where)
        oi = None
        if order is not None:
            oi = col_index(tbl, order)
        rows = [r for r in tbl.rows if matches(r, conds)]
        if oi is not None:
            rows = sorted(rows, key=lambda r: r[oi], reverse=desc)
        if limit is not None:
            rows = rows[:limit]
        out = [' | '.join(tbl.cols[i][0] for i in sel)]
        for r in rows:
            out.append(' | '.join(str(r[i]) for i in sel))
        out.append('(%d rows)' % len(rows))
        return out

    if kind == 'count':
        _, t, where = st
        tbl = get_table(db, t)
        conds = check_where(tbl, where)
        n = sum(1 for r in tbl.rows if matches(r, conds))
        return [str(n)]

    if kind == 'update':
        _, t, c, v, where = st
        tbl = get_table(db, t)
        ci = col_index(tbl, c)
        check_type(tbl.cols[ci][1], v)
        conds = check_where(tbl, where)
        n = 0
        for r in tbl.rows:
            if matches(r, conds):
                r[ci] = v[1]
                n += 1
        return ['updated %d' % n]

    if kind == 'delete':
        _, t, where = st
        tbl = get_table(db, t)
        conds = check_where(tbl, where)
        keep = [r for r in tbl.rows if not matches(r, conds)]
        n = len(tbl.rows) - len(keep)
        tbl.rows = keep
        return ['deleted %d' % n]

    raise SyntaxErr()


def main():
    db = {}
    out = []
    for raw in sys.stdin.read().split('\n'):
        line = raw.strip()
        if not line:
            continue
        try:
            out.extend(execute(line, db))
        except SyntaxErr:
            out.append('error: syntax')
        except SqlErr as e:
            out.append(e.msg)
    if out:
        sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
