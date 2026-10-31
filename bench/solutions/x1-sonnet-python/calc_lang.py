import sys, re

sys.setrecursionlimit(100000)

class Syn(Exception):
    pass

class Err(Exception):
    pass

KW = {'let', 'fn', 'print', 'if'}
TOK = re.compile(r'\s*(?:(\d+)|([A-Za-z][A-Za-z0-9_]*)|(==|!=|<=|>=|[<>+\-*/%(),=]))')

def tokenize(s):
    toks = []
    i = 0
    s = s.rstrip()
    while i < len(s):
        m = TOK.match(s, i)
        if not m:
            raise Syn()
        if m.group(1) is not None:
            toks.append(('n', int(m.group(1))))
        elif m.group(2) is not None:
            toks.append(('w', m.group(2)))
        else:
            toks.append(('o', m.group(3)))
        i = m.end()
    return toks

class Parser:
    def __init__(self, toks):
        self.t = toks
        self.i = 0
    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else (None, None)
    def next(self):
        if self.i >= len(self.t):
            raise Syn()
        self.i += 1
        return self.t[self.i - 1]
    def isop(self, o):
        return self.peek() == ('o', o)
    def expect(self, o):
        if not self.isop(o):
            raise Syn()
        self.i += 1
    def name(self):
        k, v = self.next()
        if k != 'w' or v in KW:
            raise Syn()
        return v
    def expr(self):
        a = self.add()
        k, v = self.peek()
        if k == 'o' and v in ('==', '!=', '<', '>', '<=', '>='):
            self.i += 1
            b = self.add()
            a = ('bin', v, a, b)
        return a
    def add(self):
        a = self.mul()
        while self.peek()[0] == 'o' and self.peek()[1] in ('+', '-'):
            op = self.next()[1]
            a = ('bin', op, a, self.mul())
        return a
    def mul(self):
        a = self.un()
        while self.peek()[0] == 'o' and self.peek()[1] in ('*', '/', '%'):
            op = self.next()[1]
            a = ('bin', op, a, self.un())
        return a
    def un(self):
        if self.isop('-'):
            self.i += 1
            return ('neg', self.un())
        return self.prim()
    def prim(self):
        k, v = self.next()
        if k == 'n':
            return ('num', v)
        if k == 'w':
            if v in KW and v != 'if':
                raise Syn()
            if self.isop('('):
                self.i += 1
                args = []
                if self.isop(')'):
                    self.i += 1
                else:
                    while True:
                        args.append(self.expr())
                        if self.isop(','):
                            self.i += 1
                            continue
                        self.expect(')')
                        break
                return ('call', v, args)
            if v == 'if':
                raise Syn()
            return ('var', v)
        if k == 'o' and v == '(':
            e = self.expr()
            self.expect(')')
            return e
        raise Syn()
    def done(self):
        if self.i != len(self.t):
            raise Syn()

def wrap(x):
    x &= (1 << 64) - 1
    return x - (1 << 64) if x >= (1 << 63) else x

G = {}
F = {}
depth = 0

def ev(e, loc):
    global depth
    k = e[0]
    if k == 'num':
        return wrap(e[1])
    if k == 'var':
        n = e[1]
        if loc is not None and n in loc:
            return loc[n]
        if n in G:
            return G[n]
        raise Err("unknown name " + n)
    if k == 'neg':
        return wrap(-ev(e[1], loc))
    if k == 'bin':
        op = e[1]
        a = ev(e[2], loc)
        b = ev(e[3], loc)
        if op == '+': return wrap(a + b)
        if op == '-': return wrap(a - b)
        if op == '*': return wrap(a * b)
        if op in ('/', '%'):
            if b == 0:
                raise Err("division by zero")
            q = abs(a) // abs(b)
            if (a < 0) != (b < 0):
                q = -q
            return wrap(q) if op == '/' else wrap(a - q * b)
        return int({'==': a == b, '!=': a != b, '<': a < b, '>': a > b, '<=': a <= b, '>=': a >= b}[op])
    if k == 'call':
        name, args = e[1], e[2]
        if name == 'if':
            if len(args) != 3:
                raise Err("if takes 3 arguments")
            return ev(args[1], loc) if ev(args[0], loc) != 0 else ev(args[2], loc)
        if name not in F:
            raise Err("unknown function " + name)
        ps, body = F[name]
        if len(ps) != len(args):
            raise Err("%s takes %d arguments" % (name, len(ps)))
        vals = [ev(a, loc) for a in args]
        if depth >= 200:
            raise Err("too deep")
        depth += 1
        try:
            return ev(body, dict(zip(ps, vals)))
        finally:
            depth -= 1

def main():
    out = []
    for n, line in enumerate(sys.stdin.read().split('\n'), 1):
        s = line.strip()
        if not s or s[0] == '#':
            continue
        try:
            toks = tokenize(s)
            p = Parser(toks)
            k, v = p.next()
            if (k, v) == ('w', 'let'):
                nm = p.name()
                p.expect('=')
                e = p.expr()
                p.done()
                action = ('let', nm, e)
            elif (k, v) == ('w', 'fn'):
                nm = p.name()
                p.expect('(')
                ps = []
                if p.isop(')'):
                    p.i += 1
                else:
                    while True:
                        ps.append(p.name())
                        if p.isop(','):
                            p.i += 1
                            continue
                        p.expect(')')
                        break
                p.expect('=')
                e = p.expr()
                p.done()
                action = ('fn', nm, ps, e)
            elif (k, v) == ('w', 'print'):
                es = [p.expr()]
                while p.isop(','):
                    p.i += 1
                    es.append(p.expr())
                p.done()
                action = ('print', es)
            else:
                raise Syn()
        except Syn:
            out.append("error line %d: syntax" % n)
            continue
        try:
            depth_reset()
            if action[0] == 'let':
                G[action[1]] = ev(action[2], None)
            elif action[0] == 'fn':
                F[action[1]] = (action[2], action[3])
            else:
                vals = [ev(x, None) for x in action[1]]
                out.append(" ".join(str(x) for x in vals))
        except Err as ex:
            out.append("error line %d: %s" % (n, ex))
    print("\n".join(out))

def depth_reset():
    global depth
    depth = 0

import threading
threading.stack_size(256 * 1024 * 1024)
t = threading.Thread(target=main)
t.start()
t.join()
