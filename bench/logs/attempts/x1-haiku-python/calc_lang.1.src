import re
import sys
import threading

TOKRE = re.compile(r'\s*(?:([0-9]+)|([A-Za-z][A-Za-z0-9_]*)|(==|!=|<=|>=|[<>+\-*/%()=,]))')
RESERVED = ('let', 'fn', 'print', 'if')
CMP_OPS = ('==', '!=', '<', '>', '<=', '>=')
MAX_DEPTH = 200


class SyntaxErr(Exception):
    pass


class LangErr(Exception):
    def __init__(self, msg):
        Exception.__init__(self, msg)
        self.msg = msg


def wrap(x):
    return ((x + (1 << 63)) % (1 << 64)) - (1 << 63)


def tokenize(line):
    toks = []
    pos = 0
    while True:
        m = TOKRE.match(line, pos)
        if not m:
            if line[pos:].strip() == '':
                return toks
            raise SyntaxErr()
        if m.group(1) is not None:
            toks.append(('N', m.group(1)))
        elif m.group(2) is not None:
            toks.append(('ID', m.group(2)))
        else:
            toks.append(('OP', m.group(3)))
        pos = m.end()


class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0

    def peek(self):
        if self.i < len(self.t):
            return self.t[self.i]
        return (None, None)

    def op(self, s):
        if self.peek() == ('OP', s):
            self.i += 1
            return
        raise SyntaxErr()

    def is_op(self, s):
        return self.peek() == ('OP', s)

    def name(self):
        k, v = self.peek()
        if k == 'ID' and v not in RESERVED:
            self.i += 1
            return v
        raise SyntaxErr()

    def expr(self):
        left = self.add()
        k, v = self.peek()
        if k == 'OP' and v in CMP_OPS:
            self.i += 1
            right = self.add()
            node = ('cmp', v, left, right)
            k2, v2 = self.peek()
            if k2 == 'OP' and v2 in CMP_OPS:
                raise SyntaxErr()
            return node
        return left

    def add(self):
        node = self.mul()
        while self.peek()[0] == 'OP' and self.peek()[1] in ('+', '-'):
            v = self.peek()[1]
            self.i += 1
            node = ('bin', v, node, self.mul())
        return node

    def mul(self):
        node = self.unary()
        while self.peek()[0] == 'OP' and self.peek()[1] in ('*', '/', '%'):
            v = self.peek()[1]
            self.i += 1
            node = ('bin', v, node, self.unary())
        return node

    def unary(self):
        if self.is_op('-'):
            self.i += 1
            return ('neg', self.unary())
        return self.primary()

    def args(self):
        # assumes '(' already consumed; consumes ')'
        args = []
        if self.is_op(')'):
            self.i += 1
            return args
        while True:
            args.append(self.expr())
            if self.is_op(','):
                self.i += 1
                continue
            break
        self.op(')')
        return args

    def primary(self):
        k, v = self.peek()
        if k == 'N':
            self.i += 1
            return ('num', wrap(int(v)))
        if k == 'OP' and v == '(':
            self.i += 1
            node = self.expr()
            self.op(')')
            return node
        if k == 'ID':
            if v in RESERVED:
                if v == 'if' and self.t[self.i + 1:self.i + 2] == [('OP', '(')]:
                    self.i += 2
                    return ('call', 'if', self.args())
                raise SyntaxErr()
            self.i += 1
            if self.is_op('('):
                self.i += 1
                return ('call', v, self.args())
            return ('var', v)
        raise SyntaxErr()


def parse_line(toks):
    if not toks or toks[0][0] != 'ID':
        raise SyntaxErr()
    p = Parser(toks)
    head = toks[0][1]
    if head == 'let':
        p.i = 1
        name = p.name()
        p.op('=')
        e = p.expr()
        if p.i != len(toks):
            raise SyntaxErr()
        return ('let', name, e)
    if head == 'fn':
        p.i = 1
        name = p.name()
        p.op('(')
        params = []
        if not p.is_op(')'):
            while True:
                params.append(p.name())
                if p.is_op(','):
                    p.i += 1
                    continue
                break
        p.op(')')
        p.op('=')
        e = p.expr()
        if p.i != len(toks):
            raise SyntaxErr()
        return ('fn', name, params, e)
    if head == 'print':
        p.i = 1
        exprs = [p.expr()]
        while p.is_op(','):
            p.i += 1
            exprs.append(p.expr())
        if p.i != len(toks):
            raise SyntaxErr()
        return ('print', exprs)
    raise SyntaxErr()


def tdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a < 0) == (b < 0) else -q


class Interp:
    def __init__(self):
        self.globals = {}
        self.funcs = {}
        self.depth = 0

    def ev(self, node, env):
        kind = node[0]
        if kind == 'num':
            return node[1]
        if kind == 'var':
            name = node[1]
            if name in env:
                return env[name]
            if name in self.globals:
                return self.globals[name]
            raise LangErr('unknown name %s' % name)
        if kind == 'neg':
            return wrap(-self.ev(node[1], env))
        if kind == 'bin':
            op = node[1]
            a = self.ev(node[2], env)
            b = self.ev(node[3], env)
            if op == '+':
                return wrap(a + b)
            if op == '-':
                return wrap(a - b)
            if op == '*':
                return wrap(a * b)
            if b == 0:
                raise LangErr('division by zero')
            q = tdiv(a, b)
            if op == '/':
                return wrap(q)
            return wrap(a - b * q)
        if kind == 'cmp':
            op = node[1]
            a = self.ev(node[2], env)
            b = self.ev(node[3], env)
            r = {'==': a == b, '!=': a != b, '<': a < b, '>': a > b,
                 '<=': a <= b, '>=': a >= b}[op]
            return 1 if r else 0
        if kind == 'call':
            name = node[1]
            argn = node[2]
            if name == 'if':
                if len(argn) != 3:
                    raise LangErr('if takes 3 arguments')
                c = self.ev(argn[0], env)
                return self.ev(argn[1] if c != 0 else argn[2], env)
            if name not in self.funcs:
                raise LangErr('unknown function %s' % name)
            params, body = self.funcs[name]
            if len(argn) != len(params):
                raise LangErr('%s takes %d arguments' % (name, len(params)))
            vals = [self.ev(a, env) for a in argn]
            if self.depth + 1 > MAX_DEPTH:
                raise LangErr('too deep')
            new_env = dict(zip(params, vals))
            self.depth += 1
            try:
                return self.ev(body, new_env)
            finally:
                self.depth -= 1
        raise LangErr('syntax')

    def run(self, st):
        if st[0] == 'let':
            v = self.ev(st[2], {})
            self.globals[st[1]] = v
            return None
        if st[0] == 'fn':
            self.funcs[st[1]] = (st[2], st[3])
            return None
        vals = [self.ev(e, {}) for e in st[1]]
        return ' '.join(str(v) for v in vals)


def main():
    interp = Interp()
    out = []
    for lineno, raw in enumerate(sys.stdin.read().split('\n'), 1):
        line = raw.rstrip('\r')
        if line.strip() == '' or line.lstrip().startswith('#'):
            continue
        try:
            st = parse_line(tokenize(line))
        except SyntaxErr:
            out.append('error line %d: syntax' % lineno)
            continue
        try:
            res = interp.run(st)
        except LangErr as e:
            out.append('error line %d: %s' % (lineno, e.msg))
            continue
        if res is not None:
            out.append(res)
    sys.stdout.write(''.join(l + '\n' for l in out))


def start():
    threading.stack_size(256 * 1024 * 1024)
    sys.setrecursionlimit(1000000)
    t = threading.Thread(target=main)
    t.start()
    t.join()


start()
