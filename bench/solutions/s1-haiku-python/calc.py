import sys


class DivZero(Exception):
    pass


def tokenize(s):
    toks = []
    i = 0
    while i < len(s):
        c = s[i]
        if c.isspace():
            i += 1
        elif c.isdigit():
            j = i
            while j < len(s) and s[j].isdigit():
                j += 1
            toks.append(int(s[i:j]))
            i = j
        else:
            toks.append(c)
            i += 1
    return toks


def trunc_div(a, b):
    if b == 0:
        raise DivZero()
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


class Parser:
    def __init__(self, toks):
        self.toks = toks
        self.pos = 0

    def peek(self):
        return self.toks[self.pos] if self.pos < len(self.toks) else None

    def next(self):
        t = self.peek()
        self.pos += 1
        return t

    def expr(self):
        v = self.term()
        while self.peek() in ('+', '-'):
            op = self.next()
            r = self.term()
            v = v + r if op == '+' else v - r
        return v

    def term(self):
        v = self.factor()
        while self.peek() in ('*', '/'):
            op = self.next()
            r = self.factor()
            if op == '*':
                v = v * r
            else:
                v = trunc_div(v, r)
        return v

    def factor(self):
        t = self.peek()
        if t == '-':
            self.next()
            return -self.factor()
        if t == '+':
            self.next()
            return self.factor()
        if t == '(':
            self.next()
            v = self.expr()
            if self.peek() == ')':
                self.next()
            return v
        if isinstance(t, int):
            self.next()
            return t
        raise ValueError("bad token")


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        p = Parser(tokenize(line))
        print(p.expr())
    except DivZero:
        print("error")
