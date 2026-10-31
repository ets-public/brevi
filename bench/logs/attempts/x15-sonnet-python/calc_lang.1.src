import sys, re, threading

TOK = re.compile(r"\s*(?:(\d+)|([A-Za-z][A-Za-z0-9_]*)|(==|!=|<=|>=|[<>+\-*/%(),=]))")
KW = {"let", "fn", "print", "if"}

class Syn(Exception):
    pass

class Rt(Exception):
    pass

def wrap(x):
    x &= (1 << 64) - 1
    return x - (1 << 64) if x >> 63 else x

def tokenize(s):
    pos = 0
    toks = []
    s = s.rstrip()
    while pos < len(s):
        m = TOK.match(s, pos)
        if not m:
            raise Syn()
        if m.group(1) is not None:
            toks.append(("n", int(m.group(1))))
        elif m.group(2) is not None:
            toks.append(("id", m.group(2)))
        else:
            toks.append(("op", m.group(3)))
        pos = m.end()
    return toks

class P:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else (None, None)

    def op(self, o):
        k, v = self.peek()
        return k == "op" and v == o

    def eat(self, o):
        if not self.op(o):
            raise Syn()
        self.i += 1

    def expr(self):
        l = self.add()
        k, v = self.peek()
        if k == "op" and v in ("==", "!=", "<", ">", "<=", ">="):
            self.i += 1
            r = self.add()
            l = ("bin", v, l, r)
            k, v = self.peek()
            if k == "op" and v in ("==", "!=", "<", ">", "<=", ">="):
                raise Syn()
        return l

    def add(self):
        l = self.mul()
        while True:
            k, v = self.peek()
            if k == "op" and v in ("+", "-"):
                self.i += 1
                l = ("bin", v, l, self.mul())
            else:
                return l

    def mul(self):
        l = self.un()
        while True:
            k, v = self.peek()
            if k == "op" and v in ("*", "/", "%"):
                self.i += 1
                l = ("bin", v, l, self.un())
            else:
                return l

    def un(self):
        if self.op("-"):
            self.i += 1
            return ("neg", self.un())
        return self.atom()

    def atom(self):
        k, v = self.peek()
        if k == "n":
            self.i += 1
            return ("num", wrap(v))
        if k == "id":
            if v in KW and v != "if":
                raise Syn()
            self.i += 1
            if self.op("("):
                self.i += 1
                args = []
                if not self.op(")"):
                    args.append(self.expr())
                    while self.op(","):
                        self.i += 1
                        args.append(self.expr())
                self.eat(")")
                return ("call", v, args)
            if v == "if":
                raise Syn()
            return ("var", v)
        if k == "op" and v == "(":
            self.i += 1
            e = self.expr()
            self.eat(")")
            return e
        raise Syn()

    def done(self):
        if self.i != len(self.t):
            raise Syn()

globs = {}
funcs = {}

def ev(e, loc, depth):
    t = e[0]
    if t == "num":
        return e[1]
    if t == "var":
        n = e[1]
        if loc is not None and n in loc:
            return loc[n]
        if n in globs:
            return globs[n]
        raise Rt("unknown name " + n)
    if t == "neg":
        return wrap(-ev(e[1], loc, depth))
    if t == "bin":
        op = e[1]
        a = ev(e[2], loc, depth)
        b = ev(e[3], loc, depth)
        if op == "+": return wrap(a + b)
        if op == "-": return wrap(a - b)
        if op == "*": return wrap(a * b)
        if op in ("/", "%"):
            if b == 0:
                raise Rt("division by zero")
            q = abs(a) // abs(b)
            if (a < 0) != (b < 0):
                q = -q
            if op == "/":
                return wrap(q)
            return wrap(a - q * b)
        return int({"==": a == b, "!=": a != b, "<": a < b, ">": a > b, "<=": a <= b, ">=": a >= b}[op])
    name, args = e[1], e[2]
    if name == "if":
        if len(args) != 3:
            raise Rt("if takes 3 arguments")
        c = ev(args[0], loc, depth)
        return ev(args[1] if c != 0 else args[2], loc, depth)
    if name not in funcs:
        raise Rt("unknown function " + name)
    params, body = funcs[name]
    if len(params) != len(args):
        raise Rt("%s takes %d arguments" % (name, len(params)))
    vals = [ev(a, loc, depth) for a in args]
    if depth + 1 > 200:
        raise Rt("too deep")
    return ev(body, dict(zip(params, vals)), depth + 1)

def name_ok(v):
    return v[0] == "id" and v[1] not in KW

def run_line(line):
    toks = tokenize(line)
    if not toks or toks[0][0] != "id":
        raise Syn()
    kw = toks[0][1]
    if kw == "let":
        if len(toks) < 4 or not name_ok(toks[1]) or toks[2] != ("op", "="):
            raise Syn()
        p = P(toks[3:])
        e = p.expr()
        p.done()
        return ("let", toks[1][1], e)
    if kw == "fn":
        if len(toks) < 5 or not name_ok(toks[1]) or toks[2] != ("op", "("):
            raise Syn()
        i = 3
        params = []
        if toks[i] == ("op", ")"):
            i += 1
        else:
            while True:
                if i >= len(toks) or not name_ok(toks[i]):
                    raise Syn()
                params.append(toks[i][1])
                i += 1
                if i < len(toks) and toks[i] == ("op", ","):
                    i += 1
                    continue
                if i < len(toks) and toks[i] == ("op", ")"):
                    i += 1
                    break
                raise Syn()
        if i >= len(toks) or toks[i] != ("op", "="):
            raise Syn()
        p = P(toks[i + 1:])
        e = p.expr()
        p.done()
        return ("fn", toks[1][1], params, e)
    if kw == "print":
        p = P(toks[1:])
        es = [p.expr()]
        while p.op(","):
            p.i += 1
            es.append(p.expr())
        p.done()
        return ("print", es)
    raise Syn()

def main():
    out = []
    for n, line in enumerate(sys.stdin.read().split("\n"), 1):
        s = line.strip()
        if not s or s[0] == "#":
            continue
        try:
            st = run_line(s)
            if st[0] == "let":
                globs[st[1]] = ev(st[2], None, 0)
            elif st[0] == "fn":
                funcs[st[1]] = (st[2], st[3])
            else:
                out.append(" ".join(str(ev(e, None, 0)) for e in st[1]))
        except Syn:
            out.append("error line %d: syntax" % n)
        except Rt as r:
            out.append("error line %d: %s" % (n, r))
    print("\n".join(out))

sys.setrecursionlimit(1000000)
threading.stack_size(512 * 1024 * 1024)
t = threading.Thread(target=main)
t.start()
t.join()
