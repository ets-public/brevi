import sys
import re
import threading

KEYWORDS = ('let', 'fn', 'print', 'if')
CMP_OPS = ('==', '!=', '<', '>', '<=', '>=')

TOKEN_RE = re.compile(r'\s*(?:([0-9]+)|([A-Za-z][A-Za-z0-9_]*)|(==|!=|<=|>=|[<>+\-*/%(),=]))')


class SynErr(Exception):
    pass


class RunErr(Exception):
    def __init__(self, msg):
        Exception.__init__(self, msg)
        self.msg = msg


def wrap(x):
    x &= 0xFFFFFFFFFFFFFFFF
    if x >= (1 << 63):
        x -= 1 << 64
    return x


def tokenize(s):
    toks = []
    pos = 0
    n = len(s)
    while True:
        while pos < n and s[pos] == ' ':
            pos += 1
        if pos >= n:
            break
        m = TOKEN_RE.match(s, pos)
        if not m or m.end() == pos:
            raise SynErr()
        if m.group(1) is not None:
            toks.append(('num', wrap(int(m.group(1)))))
        elif m.group(2) is not None:
            toks.append(('name', m.group(2)))
        else:
            toks.append(('op', m.group(3)))
        pos = m.end()
    return toks


class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else None

    def try_op(self, o):
        tk = self.peek()
        if tk is not None and tk == ('op', o):
            self.i += 1
            return True
        return False

    def expect(self, o):
        if not self.try_op(o):
            raise SynErr()

    def ident(self):
        tk = self.peek()
        if tk is None or tk[0] != 'name' or tk[1] in KEYWORDS:
            raise SynErr()
        self.i += 1
        return tk[1]

    def end(self):
        if self.i != len(self.t):
            raise SynErr()

    def expr(self):
        a = self.add()
        tk = self.peek()
        if tk is not None and tk[0] == 'op' and tk[1] in CMP_OPS:
            self.i += 1
            b = self.add()
            return ('cmp', tk[1], a, b)
        return a

    def add(self):
        a = self.mul()
        while True:
            tk = self.peek()
            if tk is not None and tk[0] == 'op' and tk[1] in ('+', '-'):
                self.i += 1
                b = self.mul()
                a = ('bin', tk[1], a, b)
            else:
                return a

    def mul(self):
        a = self.unary()
        while True:
            tk = self.peek()
            if tk is not None and tk[0] == 'op' and tk[1] in ('*', '/', '%'):
                self.i += 1
                b = self.unary()
                a = ('bin', tk[1], a, b)
            else:
                return a

    def unary(self):
        if self.try_op('-'):
            return ('neg', self.unary())
        return self.primary()

    def primary(self):
        tk = self.peek()
        if tk is None:
            raise SynErr()
        if tk[0] == 'num':
            self.i += 1
            return ('num', tk[1])
        if tk[0] == 'name':
            name = tk[1]
            if name in ('let', 'fn', 'print'):
                raise SynErr()
            self.i += 1
            if self.try_op('('):
                args = []
                if not self.try_op(')'):
                    args.append(self.expr())
                    while self.try_op(','):
                        args.append(self.expr())
                    self.expect(')')
                return ('call', name, args)
            if name == 'if':
                raise SynErr()
            return ('var', name)
        if self.try_op('('):
            e = self.expr()
            self.expect(')')
            return e
        raise SynErr()


def parse_line(line):
    p = Parser(tokenize(line))
    first = p.peek()
    if first == ('name', 'let'):
        p.i += 1
        name = p.ident()
        p.expect('=')
        e = p.expr()
        p.end()
        return ('let', name, e)
    if first == ('name', 'fn'):
        p.i += 1
        name = p.ident()
        p.expect('(')
        params = []
        if not p.try_op(')'):
            params.append(p.ident())
            while p.try_op(','):
                params.append(p.ident())
            p.expect(')')
        p.expect('=')
        e = p.expr()
        p.end()
        return ('fn', name, params, e)
    if first == ('name', 'print'):
        p.i += 1
        es = [p.expr()]
        while p.try_op(','):
            es.append(p.expr())
        p.end()
        return ('print', es)
    raise SynErr()


G = {}
F = {}
depth = [0]


def ev(node, frame):
    k = node[0]
    if k == 'num':
        return node[1]
    if k == 'var':
        n = node[1]
        if n in frame:
            return frame[n]
        if n in G:
            return G[n]
        raise RunErr('unknown name %s' % n)
    if k == 'neg':
        return wrap(-ev(node[1], frame))
    if k == 'bin':
        op = node[1]
        a = ev(node[2], frame)
        b = ev(node[3], frame)
        if op == '+':
            r = a + b
        elif op == '-':
            r = a - b
        elif op == '*':
            r = a * b
        else:
            if b == 0:
                raise RunErr('division by zero')
            q = abs(a) // abs(b)
            if op == '/':
                r = q if (a < 0) == (b < 0) else -q
            else:
                r = abs(a) % abs(b)
                if a < 0:
                    r = -r
        return wrap(r)
    if k == 'cmp':
        op = node[1]
        a = ev(node[2], frame)
        b = ev(node[3], frame)
        if op == '==':
            t = a == b
        elif op == '!=':
            t = a != b
        elif op == '<':
            t = a < b
        elif op == '>':
            t = a > b
        elif op == '<=':
            t = a <= b
        else:
            t = a >= b
        return 1 if t else 0
    if k == 'call':
        name = node[1]
        args = node[2]
        if name == 'if':
            if len(args) != 3:
                raise RunErr('if takes 3 arguments')
            c = ev(args[0], frame)
            return ev(args[1], frame) if c != 0 else ev(args[2], frame)
        if name not in F:
            raise RunErr('unknown function %s' % name)
        vals = [ev(a, frame) for a in args]
        params, body = F[name]
        if len(vals) != len(params):
            raise RunErr('%s takes %d arguments' % (name, len(params)))
        if depth[0] >= 200:
            raise RunErr('too deep')
        depth[0] += 1
        try:
            return ev(body, dict(zip(params, vals)))
        finally:
            depth[0] -= 1
    raise RunErr('syntax')


def main():
    out = []
    lines = sys.stdin.read().split('\n')
    for n, raw in enumerate(lines, 1):
        s = raw.strip()
        if not s or s.startswith('#'):
            continue
        try:
            st = parse_line(s)
        except SynErr:
            out.append('error line %d: syntax' % n)
            continue
        try:
            if st[0] == 'let':
                G[st[1]] = ev(st[2], {})
            elif st[0] == 'fn':
                F[st[1]] = (st[2], st[3])
            else:
                vals = [ev(e, {}) for e in st[1]]
                out.append(' '.join(str(v) for v in vals))
        except RunErr as e:
            out.append('error line %d: %s' % (n, e.msg))
    if out:
        sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    sys.setrecursionlimit(1000000)
    threading.stack_size(256 * 1024 * 1024)
    th = threading.Thread(target=main)
    th.start()
    th.join()
