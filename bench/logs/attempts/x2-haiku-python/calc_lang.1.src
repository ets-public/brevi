import sys
import re
import threading

KEYWORDS = {'let', 'fn', 'print', 'if'}
CMPOPS = {'==', '!=', '<', '>', '<=', '>='}
TOKRE = re.compile(
    r"(?P<num>[0-9]+)"
    r"|(?P<name>[A-Za-z][A-Za-z0-9_]*)"
    r"|(?P<op>==|!=|<=|>=|[<>=+\-*/%])"
    r"|(?P<punc>[(),])"
)
MAXDEPTH = 200


class Syn(Exception):
    pass


class Err(Exception):
    pass


def wrap(x):
    return ((x + 2 ** 63) % 2 ** 64) - 2 ** 63


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

    def is_punc(self, c):
        return self.peek() == ('punc', c)

    def expect_punc(self, c):
        if self.take() != ('punc', c):
            raise Syn()

    def name(self):
        k, v = self.take()
        if k != 'name' or v in KEYWORDS:
            raise Syn()
        return v

    def done(self):
        if self.i != len(self.t):
            raise Syn()


def p_expr(p):
    left = p_add(p)
    k, v = p.peek()
    if k == 'op' and v in CMPOPS:
        p.i += 1
        right = p_add(p)
        return ('cmp', v, left, right)
    return left


def p_add(p):
    left = p_mul(p)
    while p.peek() in (('op', '+'), ('op', '-')):
        op = p.take()[1]
        right = p_mul(p)
        left = ('bin', op, left, right)
    return left


def p_mul(p):
    left = p_unary(p)
    while p.peek() in (('op', '*'), ('op', '/'), ('op', '%')):
        op = p.take()[1]
        right = p_unary(p)
        left = ('bin', op, left, right)
    return left


def p_unary(p):
    if p.peek() == ('op', '-'):
        p.i += 1
        return ('neg', p_unary(p))
    return p_primary(p)


def p_args(p):
    # assumes '(' already consumed
    args = []
    if not p.is_punc(')'):
        args.append(p_expr(p))
        while p.is_punc(','):
            p.i += 1
            args.append(p_expr(p))
    p.expect_punc(')')
    return args


def p_primary(p):
    k, v = p.take()
    if k == 'num':
        return ('num', wrap(int(v)))
    if k == 'name':
        if v in KEYWORDS:
            if v == 'if' and p.is_punc('('):
                p.i += 1
                return ('call', 'if', p_args(p))
            raise Syn()
        if p.is_punc('('):
            p.i += 1
            return ('call', v, p_args(p))
        return ('name', v)
    if k == 'punc' and v == '(':
        e = p_expr(p)
        p.expect_punc(')')
        return e
    raise Syn()


def parse(line):
    p = Parser(tokenize(line))
    k, w = p.take()
    if k != 'name':
        raise Syn()
    if w == 'let':
        name = p.name()
        if p.take() != ('op', '='):
            raise Syn()
        e = p_expr(p)
        p.done()
        return ('let', name, e)
    if w == 'fn':
        name = p.name()
        p.expect_punc('(')
        params = []
        if not p.is_punc(')'):
            params.append(p.name())
            while p.is_punc(','):
                p.i += 1
                params.append(p.name())
        p.expect_punc(')')
        if p.take() != ('op', '='):
            raise Syn()
        body = p_expr(p)
        p.done()
        return ('fn', name, params, body)
    if w == 'print':
        exprs = [p_expr(p)]
        while p.is_punc(','):
            p.i += 1
            exprs.append(p_expr(p))
        p.done()
        return ('print', exprs)
    raise Syn()


class State:
    def __init__(self):
        self.G = {}
        self.F = {}
        self.depth = 0


def ev(node, env, st):
    k = node[0]
    if k == 'num':
        return node[1]
    if k == 'name':
        n = node[1]
        if n in env:
            return env[n]
        if n in st.G:
            return st.G[n]
        raise Err('unknown name %s' % n)
    if k == 'neg':
        return wrap(-ev(node[1], env, st))
    if k == 'bin':
        op = node[1]
        a = ev(node[2], env, st)
        b = ev(node[3], env, st)
        if op == '+':
            return wrap(a + b)
        if op == '-':
            return wrap(a - b)
        if op == '*':
            return wrap(a * b)
        if b == 0:
            raise Err('division by zero')
        q = abs(a) // abs(b)
        if (a < 0) != (b < 0):
            q = -q
        if op == '/':
            return wrap(q)
        return wrap(a - b * q)
    if k == 'cmp':
        op = node[1]
        a = ev(node[2], env, st)
        b = ev(node[3], env, st)
        r = {'==': a == b, '!=': a != b, '<': a < b, '>': a > b,
             '<=': a <= b, '>=': a >= b}[op]
        return 1 if r else 0
    if k == 'call':
        name, args = node[1], node[2]
        if name == 'if':
            if len(args) != 3:
                raise Err('if takes 3 arguments')
            c = ev(args[0], env, st)
            return ev(args[1] if c != 0 else args[2], env, st)
        vals = [ev(a, env, st) for a in args]
        if name not in st.F:
            raise Err('unknown function %s' % name)
        params, body = st.F[name]
        if len(vals) != len(params):
            raise Err('%s takes %d arguments' % (name, len(params)))
        if st.depth >= MAXDEPTH:
            raise Err('too deep')
        st.depth += 1
        try:
            return ev(body, dict(zip(params, vals)), st)
        finally:
            st.depth -= 1
    raise Err('syntax')


def run_stmt(stmt, st):
    k = stmt[0]
    if k == 'let':
        val = ev(stmt[2], {}, st)
        st.G[stmt[1]] = val
        return []
    if k == 'fn':
        st.F[stmt[1]] = (stmt[2], stmt[3])
        return []
    if k == 'print':
        vals = [ev(e, {}, st) for e in stmt[1]]
        return [' '.join(str(v) for v in vals)]
    return []


def main():
    lines = sys.stdin.read().splitlines()
    st = State()
    out = []
    for n, raw in enumerate(lines, 1):
        s = raw.strip()
        if not s or s.startswith('#'):
            continue
        try:
            stmt = parse(s)
        except Syn:
            out.append("error line %d: syntax" % n)
            continue
        try:
            out.extend(run_stmt(stmt, st))
        except Err as e:
            out.append("error line %d: %s" % (n, e))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == '__main__':
    sys.setrecursionlimit(100000)
    threading.stack_size(256 * 1024 * 1024)
    t = threading.Thread(target=main)
    t.start()
    t.join()
