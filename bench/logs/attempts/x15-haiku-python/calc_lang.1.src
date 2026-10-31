import re
import sys
import threading

KEYWORDS = ("let", "fn", "print", "if")
CMP_OPS = ("==", "!=", "<", ">", "<=", ">=")
TOK = re.compile(r"\s*(?:(\d+)|([A-Za-z][A-Za-z0-9_]*)|(==|!=|<=|>=|[-+*/%<>=(),]))")
MAX_DEPTH = 200
MASK = (1 << 64) - 1


class SynErr(Exception):
    pass


class CalcErr(Exception):
    pass


def wrap(x):
    x &= MASK
    if x >= 1 << 63:
        x -= 1 << 64
    return x


def tokenize(s):
    toks = []
    pos = 0
    n = len(s)
    while True:
        while pos < n and s[pos].isspace():
            pos += 1
        if pos >= n:
            break
        m = TOK.match(s, pos)
        if not m:
            raise SynErr()
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

    def take(self):
        tok = self.peek()
        if tok[0] is None:
            raise SynErr()
        self.i += 1
        return tok

    def at_op(self, o):
        return self.peek() == ("op", o)

    def expect_op(self, o):
        if self.take() != ("op", o):
            raise SynErr()

    def name(self):
        k, v = self.take()
        if k != "name" or v in KEYWORDS:
            raise SynErr()
        return v

    def end(self):
        if self.i != len(self.t):
            raise SynErr()

    def expr(self):
        left = self.add()
        k, v = self.peek()
        if k == "op" and v in CMP_OPS:
            self.take()
            right = self.add()
            return ("bin", v, left, right)
        return left

    def add(self):
        node = self.mul()
        while self.peek() in (("op", "+"), ("op", "-")):
            op = self.take()[1]
            node = ("bin", op, node, self.mul())
        return node

    def mul(self):
        node = self.unary()
        while self.peek() in (("op", "*"), ("op", "/"), ("op", "%")):
            op = self.take()[1]
            node = ("bin", op, node, self.unary())
        return node

    def unary(self):
        if self.at_op("-"):
            self.take()
            return ("neg", self.unary())
        return self.primary()

    def primary(self):
        k, v = self.peek()
        if k == "num":
            self.take()
            return ("num", wrap(v))
        if k == "name":
            if v in ("let", "fn", "print"):
                raise SynErr()
            self.take()
            if self.at_op("("):
                self.take()
                args = []
                if not self.at_op(")"):
                    args.append(self.expr())
                    while self.at_op(","):
                        self.take()
                        args.append(self.expr())
                self.expect_op(")")
                return ("call", v, args)
            if v == "if":
                raise SynErr()
            return ("var", v)
        if k == "op" and v == "(":
            self.take()
            node = self.expr()
            self.expect_op(")")
            return node
        raise SynErr()


def parse(line):
    p = Parser(tokenize(line))
    k, v = p.take()
    if k != "name":
        raise SynErr()
    if v == "let":
        name = p.name()
        p.expect_op("=")
        node = p.expr()
        p.end()
        return ("let", name, node)
    if v == "fn":
        name = p.name()
        p.expect_op("(")
        params = []
        if not p.at_op(")"):
            params.append(p.name())
            while p.at_op(","):
                p.take()
                params.append(p.name())
        p.expect_op(")")
        p.expect_op("=")
        body = p.expr()
        p.end()
        return ("fn", name, params, body)
    if v == "print":
        exprs = [p.expr()]
        while p.at_op(","):
            p.take()
            exprs.append(p.expr())
        p.end()
        return ("print", exprs)
    raise SynErr()


class Interp:
    def __init__(self):
        self.globals = {}
        self.funcs = {}
        self.depth = 0

    def ev(self, node, env):
        kind = node[0]
        if kind == "num":
            return node[1]
        if kind == "var":
            name = node[1]
            if env is not None and name in env:
                return env[name]
            if name in self.globals:
                return self.globals[name]
            raise CalcErr("unknown name %s" % name)
        if kind == "neg":
            return wrap(-self.ev(node[1], env))
        if kind == "bin":
            op = node[1]
            a = self.ev(node[2], env)
            b = self.ev(node[3], env)
            if op == "+":
                return wrap(a + b)
            if op == "-":
                return wrap(a - b)
            if op == "*":
                return wrap(a * b)
            if op in ("/", "%"):
                if b == 0:
                    raise CalcErr("division by zero")
                q = abs(a) // abs(b)
                if (a < 0) != (b < 0):
                    q = -q
                if op == "/":
                    return wrap(q)
                return wrap(a - b * q)
            if op == "==":
                return 1 if a == b else 0
            if op == "!=":
                return 1 if a != b else 0
            if op == "<":
                return 1 if a < b else 0
            if op == ">":
                return 1 if a > b else 0
            if op == "<=":
                return 1 if a <= b else 0
            return 1 if a >= b else 0
        if kind == "call":
            name, args = node[1], node[2]
            if name == "if":
                if len(args) != 3:
                    raise CalcErr("if takes 3 arguments")
                c = self.ev(args[0], env)
                if c != 0:
                    return self.ev(args[1], env)
                return self.ev(args[2], env)
            if name not in self.funcs:
                raise CalcErr("unknown function %s" % name)
            params, body = self.funcs[name]
            if len(args) != len(params):
                raise CalcErr("%s takes %d arguments" % (name, len(params)))
            vals = [self.ev(a, env) for a in args]
            if self.depth >= MAX_DEPTH:
                raise CalcErr("too deep")
            frame = dict(zip(params, vals))
            self.depth += 1
            try:
                return self.ev(body, frame)
            finally:
                self.depth -= 1
        raise CalcErr("syntax")


def run(lines_out):
    interp = Interp()
    out = []
    for lineno, raw in enumerate(sys.stdin, 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        try:
            st = parse(line)
        except SynErr:
            out.append("error line %d: syntax" % lineno)
            continue
        try:
            if st[0] == "let":
                interp.globals[st[1]] = interp.ev(st[2], None)
            elif st[0] == "fn":
                interp.funcs[st[1]] = (st[2], st[3])
            else:
                vals = [interp.ev(e, None) for e in st[1]]
                out.append(" ".join(str(v) for v in vals))
        except CalcErr as e:
            out.append("error line %d: %s" % (lineno, e))
    lines_out.extend(out)


def main():
    sys.setrecursionlimit(1000000)
    threading.stack_size(512 * 1024 * 1024)
    result = []
    t = threading.Thread(target=run, args=(result,))
    t.start()
    t.join()
    if result:
        sys.stdout.write("\n".join(result) + "\n")


if __name__ == "__main__":
    main()
