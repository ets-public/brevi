import sys, re, threading

TOK = re.compile(r"\s*(?:([0-9]+)|([A-Za-z][A-Za-z0-9_]*)|(==|!=|<=|>=|[<>+\-*/%(),=]))")
KW = {'let', 'fn', 'print', 'if'}
CMP = {'==', '!=', '<', '>', '<=', '>='}

class Err(Exception):
    pass

def tokenize(s):
    toks = []
    pos = 0
    s = s.rstrip()
    while pos < len(s):
        m = TOK.match(s, pos)
        if not m:
            raise Err("syntax")
        if m.group(1) is not None:
            toks.append(('n', int(m.group(1))))
        elif m.group(2) is not None:
            toks.append(('id', m.group(2)))
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

    def next(self):
        tk = self.peek()
        if tk is None:
            raise Err("syntax")
        self.i += 1
        return tk

    def isop(self, op):
        tk = self.peek()
        return tk is not None and tk[0] == 'op' and tk[1] == op

    def expect(self, op):
        if not self.isop(op):
            raise Err("syntax")
        self.i += 1

    def name(self):
        tk = self.next()
        if tk[0] != 'id' or tk[1] in KW:
            raise Err("syntax")
        return tk[1]

    def end(self):
        if self.peek() is not None:
            raise Err("syntax")

    def expr(self):
        left = self.add()
        tk = self.peek()
        if tk and tk[0] == 'op' and tk[1] in CMP:
            self.i += 1
            right = self.add()
            left = ('bin', tk[1], left, right)
            tk = self.peek()
            if tk and tk[0] == 'op' and tk[1] in CMP:
                raise Err("syntax")
        return left

    def add(self):
        left = self.mul()
        while True:
            tk = self.peek()
            if tk and tk[0] == 'op' and tk[1] in ('+', '-'):
                self.i += 1
                left = ('bin', tk[1], left, self.mul())
            else:
                return left

    def mul(self):
        left = self.unary()
        while True:
            tk = self.peek()
            if tk and tk[0] == 'op' and tk[1] in ('*', '/', '%'):
                self.i += 1
                left = ('bin', tk[1], left, self.unary())
            else:
                return left

    def unary(self):
        if self.isop('-'):
            self.i += 1
            return ('neg', self.unary())
        return self.primary()

    def primary(self):
        tk = self.next()
        if tk[0] == 'n':
            return ('num', tk[1])
        if tk[0] == 'id':
            nm = tk[1]
            if nm in KW and nm != 'if':
                raise Err("syntax")
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
                return ('call', nm, args)
            if nm == 'if':
                raise Err("syntax")
            return ('var', nm)
        if tk == ('op', '('):
            e = self.expr()
            self.expect(')')
            return e
        raise Err("syntax")

def wrap(x):
    return (x + 2**63) % 2**64 - 2**63

globs = {}
funcs = {}
depth = 0

def ev(e, env):
    global depth
    k = e[0]
    if k == 'num':
        return wrap(e[1])
    if k == 'var':
        n = e[1]
        if n in env:
            return env[n]
        if n in globs:
            return globs[n]
        raise Err("unknown name " + n)
    if k == 'neg':
        return wrap(-ev(e[1], env))
    if k == 'bin':
        op = e[1]
        a = ev(e[2], env)
        b = ev(e[3], env)
        if op == '+': return wrap(a + b)
        if op == '-': return wrap(a - b)
        if op == '*': return wrap(a * b)
        if op in ('/', '%'):
            if b == 0:
                raise Err("division by zero")
            q = abs(a) // abs(b)
            if (a < 0) != (b < 0):
                q = -q
            if op == '/':
                return wrap(q)
            return wrap(a - q * b)
        if op == '==': return int(a == b)
        if op == '!=': return int(a != b)
        if op == '<': return int(a < b)
        if op == '>': return int(a > b)
        if op == '<=': return int(a <= b)
        if op == '>=': return int(a >= b)
    if k == 'call':
        nm, args = e[1], e[2]
        if nm == 'if':
            if len(args) != 3:
                raise Err("if takes 3 arguments")
            c = ev(args[0], env)
            return ev(args[1] if c != 0 else args[2], env)
        if nm not in funcs:
            raise Err("unknown function " + nm)
        params, body = funcs[nm]
        if len(params) != len(args):
            raise Err("%s takes %d arguments" % (nm, len(params)))
        vals = [ev(a, env) for a in args]
        if depth >= 200:
            raise Err("too deep")
        depth += 1
        try:
            return ev(body, dict(zip(params, vals)))
        finally:
            depth -= 1
    raise Err("syntax")

def run_line(line, out):
    p = Parser(tokenize(line))
    kw = p.next()
    if kw == ('id', 'let'):
        nm = p.name()
        p.expect('=')
        e = p.expr()
        p.end()
        v = ev(e, {})
        globs[nm] = v
    elif kw == ('id', 'fn'):
        nm = p.name()
        p.expect('(')
        params = []
        if p.isop(')'):
            p.i += 1
        else:
            while True:
                params.append(p.name())
                if p.isop(','):
                    p.i += 1
                    continue
                p.expect(')')
                break
        p.expect('=')
        e = p.expr()
        p.end()
        funcs[nm] = (params, e)
    elif kw == ('id', 'print'):
        es = [p.expr()]
        while p.isop(','):
            p.i += 1
            es.append(p.expr())
        p.end()
        vals = [ev(e, {}) for e in es]
        out.append(' '.join(str(v) for v in vals))
    else:
        raise Err("syntax")

def main():
    out = []
    for n, line in enumerate(sys.stdin.read().split('\n'), 1):
        s = line.strip()
        if not s or s[0] == '#':
            continue
        try:
            run_line(s, out)
        except Err as e:
            out.append("error line %d: %s" % (n, e))
        except RecursionError:
            out.append("error line %d: too deep" % n)
    print('\n'.join(out))

sys.setrecursionlimit(200000)
threading.stack_size(512 * 1024 * 1024)
t = threading.Thread(target=main)
t.start()
t.join()
