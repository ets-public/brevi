import re
import sys

TOK = re.compile(r"'((?:[^']|'')*)'|(-?\d+)|([A-Za-z][A-Za-z0-9_]*)|(!=|<=|>=|[=<>(),*])")
OPS = ("=", "!=", "<", ">", "<=", ">=")


class SynErr(Exception):
    pass


class DbErr(Exception):
    pass


def tokenize(s):
    toks = []
    pos = 0
    n = len(s)
    while pos < n:
        if s[pos].isspace():
            pos += 1
            continue
        m = TOK.match(s, pos)
        if not m:
            raise SynErr()
        if m.group(1) is not None:
            toks.append(("str", m.group(1).replace("''", "'")))
        elif m.group(2) is not None:
            toks.append(("num", int(m.group(2))))
        elif m.group(3) is not None:
            toks.append(("id", m.group(3)))
        else:
            toks.append(("op", m.group(4)))
        pos = m.end()
    return toks


class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def take(self):
        if self.i >= len(self.t):
            raise SynErr()
        tok = self.t[self.i]
        self.i += 1
        return tok

    def at_kw(self, word):
        return self.i < len(self.t) and self.t[self.i] == ("id", word)

    def at_op(self, o):
        return self.i < len(self.t) and self.t[self.i] == ("op", o)

    def kw(self, word):
        if self.take() != ("id", word):
            raise SynErr()

    def op(self, o):
        if self.take() != ("op", o):
            raise SynErr()

    def name(self):
        k, v = self.take()
        if k != "id":
            raise SynErr()
        return v

    def value(self):
        k, v = self.take()
        if k not in ("str", "num"):
            raise SynErr()
        return v

    def end(self):
        if self.i != len(self.t):
            raise SynErr()

    def cond(self):
        conds = []
        while True:
            col = self.name()
            k, v = self.take()
            if k != "op" or v not in OPS:
                raise SynErr()
            val = self.value()
            conds.append((col, v, val))
            if self.at_kw("AND"):
                self.take()
                continue
            break
        return conds

    def opt_where(self):
        if self.at_kw("WHERE"):
            self.take()
            return self.cond()
        return []


def parse(line):
    p = Parser(tokenize(line))
    head = p.take()
    if head == ("id", "CREATE"):
        p.kw("TABLE")
        tname = p.name()
        p.op("(")
        cols = []
        while True:
            cname = p.name()
            k, v = p.take()
            if k != "id" or v not in ("INT", "TEXT"):
                raise SynErr()
            cols.append((cname, v))
            if p.at_op(","):
                p.take()
                continue
            break
        p.op(")")
        p.end()
        return ("create", tname, cols)
    if head == ("id", "INSERT"):
        p.kw("INTO")
        tname = p.name()
        p.kw("VALUES")
        p.op("(")
        vals = [p.value()]
        while p.at_op(","):
            p.take()
            vals.append(p.value())
        p.op(")")
        p.end()
        return ("insert", tname, vals)
    if head == ("id", "SELECT"):
        if p.at_kw("COUNT"):
            p.take()
            p.op("(")
            p.op("*")
            p.op(")")
            p.kw("FROM")
            tname = p.name()
            where = p.opt_where()
            p.end()
            return ("count", tname, where)
        if p.at_op("*"):
            p.take()
            cols = "*"
        else:
            cols = [p.name()]
            while p.at_op(","):
                p.take()
                cols.append(p.name())
        p.kw("FROM")
        tname = p.name()
        where = p.opt_where()
        order = None
        if p.at_kw("ORDER"):
            p.take()
            p.kw("BY")
            ocol = p.name()
            desc = False
            if p.at_kw("DESC"):
                p.take()
                desc = True
            order = (ocol, desc)
        limit = None
        if p.at_kw("LIMIT"):
            p.take()
            k, v = p.take()
            if k != "num" or v < 0:
                raise SynErr()
            limit = v
        p.end()
        return ("select", tname, cols, where, order, limit)
    if head == ("id", "UPDATE"):
        tname = p.name()
        p.kw("SET")
        col = p.name()
        p.op("=")
        val = p.value()
        where = p.opt_where()
        p.end()
        return ("update", tname, col, val, where)
    if head == ("id", "DELETE"):
        p.kw("FROM")
        tname = p.name()
        where = p.opt_where()
        p.end()
        return ("delete", tname, where)
    raise SynErr()


def colindex(table, name):
    for i, (cn, _) in enumerate(table["cols"]):
        if cn == name:
            return i
    raise DbErr("error: no column %s" % name)


def check_type(table, idx, val):
    ctype = table["cols"][idx][1]
    if ctype == "INT" and not isinstance(val, int):
        raise DbErr("error: type mismatch")
    if ctype == "TEXT" and not isinstance(val, str):
        raise DbErr("error: type mismatch")


def resolve_conds(table, conds):
    out = []
    for col, op, val in conds:
        idx = colindex(table, col)
        check_type(table, idx, val)
        out.append((idx, op, val))
    return out


def row_match(row, conds):
    for idx, op, val in conds:
        a = row[idx]
        if op == "=":
            ok = a == val
        elif op == "!=":
            ok = a != val
        elif op == "<":
            ok = a < val
        elif op == ">":
            ok = a > val
        elif op == "<=":
            ok = a <= val
        else:
            ok = a >= val
        if not ok:
            return False
    return True


def fmt(v):
    return str(v)


def execute(db, st):
    kind = st[0]
    if kind == "create":
        _, tname, cols = st
        if tname in db:
            raise DbErr("error: table %s exists" % tname)
        db[tname] = {"cols": cols, "rows": []}
        return "ok"

    tname = st[1]
    if tname not in db:
        raise DbErr("error: no table %s" % tname)
    table = db[tname]

    if kind == "insert":
        vals = st[2]
        if len(vals) != len(table["cols"]):
            raise DbErr("error: type mismatch")
        for i, v in enumerate(vals):
            check_type(table, i, v)
        table["rows"].append(list(vals))
        return "ok"

    if kind == "count":
        conds = resolve_conds(table, st[2])
        n = sum(1 for r in table["rows"] if row_match(r, conds))
        return str(n)

    if kind == "select":
        _, _, cols, where, order, limit = st
        if cols == "*":
            idxs = list(range(len(table["cols"])))
            names = [c for c, _ in table["cols"]]
        else:
            idxs = []
            for c in cols:
                idxs.append(colindex(table, c))
            names = list(cols)
        conds = resolve_conds(table, where)
        rows = [r for r in table["rows"] if row_match(r, conds)]
        if order is not None:
            oidx = colindex(table, order[0])
            rows = sorted(rows, key=lambda r: r[oidx], reverse=order[1])
        if limit is not None:
            rows = rows[:limit]
        lines = [" | ".join(names)]
        for r in rows:
            lines.append(" | ".join(fmt(r[i]) for i in idxs))
        lines.append("(%d rows)" % len(rows))
        return "\n".join(lines)

    if kind == "update":
        _, _, col, val, where = st
        idx = colindex(table, col)
        check_type(table, idx, val)
        conds = resolve_conds(table, where)
        n = 0
        for r in table["rows"]:
            if row_match(r, conds):
                r[idx] = val
                n += 1
        return "updated %d" % n

    if kind == "delete":
        _, _, where = st
        conds = resolve_conds(table, where)
        keep = [r for r in table["rows"] if not row_match(r, conds)]
        n = len(table["rows"]) - len(keep)
        table["rows"] = keep
        return "deleted %d" % n

    raise SynErr()


def main():
    db = {}
    out = []
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        try:
            st = parse(line)
        except SynErr:
            out.append("error: syntax")
            continue
        try:
            out.append(execute(db, st))
        except DbErr as e:
            out.append(str(e))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
