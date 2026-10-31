import operator
import re
import sys

TOK = re.compile(r"\s*(?:('(?:[^']|'')*')|(-?[0-9]+)|([A-Za-z][A-Za-z0-9_]*)|(<=|>=|!=|[(),=<>*]))")

OPS = {
    '=': operator.eq,
    '!=': operator.ne,
    '<': operator.lt,
    '>': operator.gt,
    '<=': operator.le,
    '>=': operator.ge,
}


class SyntaxErr(Exception):
    pass


class SqlErr(Exception):
    def __init__(self, msg):
        Exception.__init__(self, msg)
        self.msg = msg


def tokenize(line):
    toks = []
    pos = 0
    while True:
        m = TOK.match(line, pos)
        if not m:
            if line[pos:].strip() == '':
                return toks
            raise SyntaxErr()
        if m.group(1) is not None:
            toks.append(('S', m.group(1)[1:-1].replace("''", "'")))
        elif m.group(2) is not None:
            toks.append(('I', int(m.group(2))))
        elif m.group(3) is not None:
            toks.append(('W', m.group(3)))
        else:
            toks.append(('P', m.group(4)))
        pos = m.end()


class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def peek(self):
        if self.i < len(self.t):
            return self.t[self.i]
        return (None, None)

    def kw(self, word):
        if self.peek() == ('W', word):
            self.i += 1
            return
        raise SyntaxErr()

    def opt(self, word):
        if self.peek() == ('W', word):
            self.i += 1
            return True
        return False

    def punct(self, p):
        if self.peek() == ('P', p):
            self.i += 1
            return
        raise SyntaxErr()

    def name(self):
        k, v = self.peek()
        if k == 'W':
            self.i += 1
            return v
        raise SyntaxErr()

    def value(self):
        k, v = self.peek()
        if k == 'I':
            self.i += 1
            return ('INT', v)
        if k == 'S':
            self.i += 1
            return ('TEXT', v)
        raise SyntaxErr()

    def done(self):
        if self.i != len(self.t):
            raise SyntaxErr()

    def cond(self):
        conds = []
        while True:
            col = self.name()
            k, op = self.peek()
            if k != 'P' or op not in OPS:
                raise SyntaxErr()
            self.i += 1
            val = self.value()
            conds.append((col, op, val))
            if not self.opt('AND'):
                break
        return conds

    def opt_where(self):
        if self.opt('WHERE'):
            return self.cond()
        return []


def parse(toks):
    if not toks or toks[0][0] != 'W':
        raise SyntaxErr()
    p = Parser(toks)
    head = toks[0][1]
    if head == 'CREATE':
        p.kw('CREATE')
        p.kw('TABLE')
        tname = p.name()
        p.punct('(')
        cols = []
        while True:
            cname = p.name()
            ctype = p.name()
            if ctype not in ('INT', 'TEXT'):
                raise SyntaxErr()
            cols.append((cname, ctype))
            if p.peek() == ('P', ','):
                p.i += 1
                continue
            break
        p.punct(')')
        p.done()
        return ('create', tname, cols)
    if head == 'INSERT':
        p.kw('INSERT')
        p.kw('INTO')
        tname = p.name()
        p.kw('VALUES')
        p.punct('(')
        vals = []
        while True:
            vals.append(p.value())
            if p.peek() == ('P', ','):
                p.i += 1
                continue
            break
        p.punct(')')
        p.done()
        return ('insert', tname, vals)
    if head == 'SELECT':
        p.kw('SELECT')
        if p.peek() == ('W', 'COUNT'):
            p.i += 1
            p.punct('(')
            p.punct('*')
            p.punct(')')
            p.kw('FROM')
            tname = p.name()
            conds = p.opt_where()
            p.done()
            return ('count', tname, conds)
        if p.peek() == ('P', '*'):
            p.i += 1
            cols = '*'
        else:
            cols = [p.name()]
            while p.peek() == ('P', ','):
                p.i += 1
                cols.append(p.name())
        p.kw('FROM')
        tname = p.name()
        conds = p.opt_where()
        order = None
        desc = False
        if p.opt('ORDER'):
            p.kw('BY')
            ocol = p.name()
            order = ocol
            desc = p.opt('DESC')
        limit = None
        if p.opt('LIMIT'):
            k, v = p.peek()
            if k != 'I' or v < 0:
                raise SyntaxErr()
            p.i += 1
            limit = v
        p.done()
        return ('select', cols, tname, conds, order, desc, limit)
    if head == 'UPDATE':
        p.kw('UPDATE')
        tname = p.name()
        p.kw('SET')
        col = p.name()
        p.punct('=')
        val = p.value()
        conds = p.opt_where()
        p.done()
        return ('update', tname, col, val, conds)
    if head == 'DELETE':
        p.kw('DELETE')
        p.kw('FROM')
        tname = p.name()
        conds = p.opt_where()
        p.done()
        return ('delete', tname, conds)
    raise SyntaxErr()


def get_table(db, tname):
    if tname not in db:
        raise SqlErr('error: no table %s' % tname)
    return db[tname]


def colidx(tab, cname):
    for i, (n, _t) in enumerate(tab['cols']):
        if n == cname:
            return i
    raise SqlErr('error: no column %s' % cname)


def resolve(tab, conds):
    return [(colidx(tab, c), op, val) for (c, op, val) in conds]


def check_types(tab, cres):
    for i, _op, (t, _v) in cres:
        if tab['cols'][i][1] != t:
            raise SqlErr('error: type mismatch')


def filter_rows(tab, cres):
    out = []
    for r in tab['rows']:
        if all(OPS[op](r[i], v) for (i, op, (_t, v)) in cres):
            out.append(r)
    return out


def execute(db, st):
    kind = st[0]
    if kind == 'create':
        _, tname, cols = st
        if tname in db:
            raise SqlErr('error: table %s exists' % tname)
        db[tname] = {'cols': cols, 'rows': []}
        return ['ok']
    if kind == 'insert':
        _, tname, vals = st
        tab = get_table(db, tname)
        if len(vals) != len(tab['cols']):
            raise SqlErr('error: type mismatch')
        for (t, _v), (_cn, ct) in zip(vals, tab['cols']):
            if t != ct:
                raise SqlErr('error: type mismatch')
        tab['rows'].append([v for (_t, v) in vals])
        return ['ok']
    if kind == 'select':
        _, cols, tname, conds, order, desc, limit = st
        tab = get_table(db, tname)
        if cols == '*':
            cidx = list(range(len(tab['cols'])))
            cnames = [n for (n, _t) in tab['cols']]
        else:
            cidx = [colidx(tab, c) for c in cols]
            cnames = cols
        cres = resolve(tab, conds)
        oidx = colidx(tab, order) if order is not None else None
        check_types(tab, cres)
        rows = filter_rows(tab, cres)
        if oidx is not None:
            rows = sorted(rows, key=lambda r: r[oidx], reverse=desc)
        if limit is not None:
            rows = rows[:limit]
        out = [' | '.join(cnames)]
        for r in rows:
            out.append(' | '.join(str(r[i]) for i in cidx))
        out.append('(%d rows)' % len(rows))
        return out
    if kind == 'count':
        _, tname, conds = st
        tab = get_table(db, tname)
        cres = resolve(tab, conds)
        check_types(tab, cres)
        return [str(len(filter_rows(tab, cres)))]
    if kind == 'update':
        _, tname, col, val, conds = st
        tab = get_table(db, tname)
        si = colidx(tab, col)
        cres = resolve(tab, conds)
        if tab['cols'][si][1] != val[0]:
            raise SqlErr('error: type mismatch')
        check_types(tab, cres)
        n = 0
        for r in tab['rows']:
            if all(OPS[op](r[i], v) for (i, op, (_t, v)) in cres):
                r[si] = val[1]
                n += 1
        return ['updated %d' % n]
    if kind == 'delete':
        _, tname, conds = st
        tab = get_table(db, tname)
        cres = resolve(tab, conds)
        check_types(tab, cres)
        keep = []
        n = 0
        for r in tab['rows']:
            if all(OPS[op](r[i], v) for (i, op, (_t, v)) in cres):
                n += 1
            else:
                keep.append(r)
        tab['rows'] = keep
        return ['deleted %d' % n]
    raise SyntaxErr()


def main():
    db = {}
    out = []
    for raw in sys.stdin.read().split('\n'):
        line = raw.rstrip('\r')
        if line.strip() == '':
            continue
        try:
            st = parse(tokenize(line))
        except SyntaxErr:
            out.append('error: syntax')
            continue
        try:
            out.extend(execute(db, st))
        except SqlErr as e:
            out.append(e.msg)
    sys.stdout.write('\n'.join(out) + ('\n' if out else ''))


main()
