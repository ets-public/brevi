import sys, re
sys.setrecursionlimit(100000)

class Err(Exception):
    pass

KW = {"let", "fn", "print", "if"}
TOK = re.compile(r"\s*(?:(\d+)|([A-Za-z][A-Za-z0-9_]*)|(==|!=|<=|>=|[<>+\-*/%(),=]))")

def tokenize(s):
    toks = []
    pos = 0
    s = s.rstrip()
    while pos < len(s):
        m = TOK.match(s, pos)
        if not m:
            raise Err("syntax")
        if m.group(1) is not None:
            toks.append(("num", int(m.group(1))))
        elif m.group(2) is not None:
            toks.append(("name", m.group(2)))
        else:
            toks.append(("op", m.group(3)))
        pos = m.end()
    return toks

class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0
    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else (None, None)
    def next(self):
        tk = self.peek()
        self.i += 1
        return tk
    def isop(self, o):
        return self.peek() == ("op", o)
    def expect(self, o):
        if not self.isop(o):
            raise Err("syntax")
        self.i += 1
    def name(self):
        k, v = self.next()
        if k != "name" or v in KW:
            raise Err("syntax")
        return v
    def expr(self):
        a = self.add()
        k, v = self.peek()
        if k == "op" and v in ("==", "!=", "<", ">", "<=", ">="):
            self.i += 1
            b = self.add()
            a = ("bin", v, a, b)
            k, v = self.peek()
            if k == "op" and v in ("==", "!=", "<", ">", "<=", ">="):
                raise Err("syntax")
        return a
    def add(self):
        a = self.mul()
        while self.peek()[0] == "op" and self.peek()[1] in ("+", "-"):
            o = self.next()[1]
            a = ("bin", o, a, self.mul())
        return a
    def mul(self):
        a = self.unary()
        while self.peek()[0] == "op" and self.peek()[1] in ("*", "/", "%"):
            o = self.next()[1]
            a = ("bin", o, a, self.unary())
        return a
    def unary(self):
        if self.isop("-"):
            self.i += 1
            return ("neg", self.unary())
        return self.atom()
    def atom(self):
        k, v = self.next()
        if k == "num":
            return ("num", v)
        if k == "op" and v == "(":
            e = self.expr()
            self.expect(")")
            return e
        if k == "name":
            if v in KW and v != "if":
                raise Err("syntax")
            if self.isop("("):
                self.i += 1
                args = []
                if self.isop(")"):
                    self.i += 1
                else:
                    while True:
                        args.append(self.expr())
                        if self.isop(","):
                            self.i += 1
                            continue
                        self.expect(")")
                        break
                return ("call", v, args)
            if v == "if":
                raise Err("syntax")
            return ("var", v)
        raise Err("syntax")

def wrap(x):
    x &= (1 << 64) - 1
    return x - (1 << 64) if x >= (1 << 63) else x

G = {}
F = {}
depth = 0

def ev(e, loc):
    global depth
    k = e[0]
    if k == "num":
        return wrap(e[1])
    if k == "var":
        n = e[1]
        if loc is not None and n in loc:
            return loc[n]
        if n in G:
            return G[n]
        raise Err("unknown name " + n)
    if k == "neg":
        return wrap(-ev(e[1], loc))
    if k == "bin":
        o = e[1]
        a = ev(e[2], loc)
        b = ev(e[3], loc)
        if o == "+": return wrap(a + b)
        if o == "-": return wrap(a - b)
        if o == "*": return wrap(a * b)
        if o in ("/", "%"):
            if b == 0:
                raise Err("division by zero")
            q = abs(a) // abs(b)
            if (a < 0) != (b < 0):
                q = -q
            if o == "/":
                return wrap(q)
            return wrap(a - q * b)
        return int({"==": a == b, "!=": a != b, "<": a < b, ">": a > b,
                    "<=": a <= b, ">=": a >= b}[o])
    if k == "call":
        n, args = e[1], e[2]
        if n == "if":
            if len(args) != 3:
                raise Err("if takes 3 arguments")
            c = ev(args[0], loc)
            return ev(args[1] if c != 0 else args[2], loc)
        if n not in F:
            raise Err("unknown function " + n)
        ps, body = F[n]
        if len(ps) != len(args):
            raise Err("%s takes %d arguments" % (n, len(ps)))
        vals = [ev(a, loc) for a in args]
        if depth >= 200:
            raise Err("too deep")
        depth += 1
        try:
            return ev(body, dict(zip(ps, vals)))
        finally:
            depth -= 1

def run(line, out):
    global depth
    toks = tokenize(line)
    if not toks:
        return
    p = Parser(toks)
    k, v = toks[0]
    if k == "name" and v == "let":
        p.i = 1
        n = p.name()
        p.expect("=")
        e = p.expr()
        if p.i != len(toks):
            raise Err("syntax")
        depth = 0
        G[n] = ev(e, None)
    elif k == "name" and v == "fn":
        p.i = 1
        n = p.name()
        p.expect("(")
        ps = []
        if p.isop(")"):
            p.i += 1
        else:
            while True:
                ps.append(p.name())
                if p.isop(","):
                    p.i += 1
                    continue
                p.expect(")")
                break
        p.expect("=")
        e = p.expr()
        if p.i != len(toks):
            raise Err("syntax")
        F[n] = (ps, e)
    elif k == "name" and v == "print":
        p.i = 1
        es = [p.expr()]
        while p.isop(","):
            p.i += 1
            es.append(p.expr())
        if p.i != len(toks):
            raise Err("syntax")
        depth = 0
        vals = [ev(e, None) for e in es]
        out.append(" ".join(str(x) for x in vals))
    else:
        raise Err("syntax")

def main():
    out = []
    for n, line in enumerate(sys.stdin.read().split("\n"), 1):
        s = line.strip()
        if not s or s[0] == "#":
            continue
        try:
            run(line, out)
        except Err as e:
            out.append("error line %d: %s" % (n, e))
    print("\n".join(out))

main()
