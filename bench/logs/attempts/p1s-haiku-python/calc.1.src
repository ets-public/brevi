import sys


class DivZero(Exception):
    pass


def tokenize(s):
    tokens = []
    i = 0
    while i < len(s):
        c = s[i]
        if c.isspace():
            i += 1
        elif c.isdigit():
            j = i
            while j < len(s) and s[j].isdigit():
                j += 1
            tokens.append(int(s[i:j]))
            i = j
        else:
            tokens.append(c)
            i += 1
    return tokens


def tdiv(a, b):
    if b == 0:
        raise DivZero()
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


class Parser:
    def __init__(self, tokens):
        self.t = tokens
        self.p = 0

    def peek(self):
        return self.t[self.p] if self.p < len(self.t) else None

    def expr(self):
        v = self.term()
        while self.peek() in ('+', '-'):
            op = self.t[self.p]
            self.p += 1
            r = self.term()
            v = v + r if op == '+' else v - r
        return v

    def term(self):
        v = self.unary()
        while self.peek() in ('*', '/'):
            op = self.t[self.p]
            self.p += 1
            r = self.unary()
            v = v * r if op == '*' else tdiv(v, r)
        return v

    def unary(self):
        if self.peek() == '-':
            self.p += 1
            return -self.unary()
        if self.peek() == '+':
            self.p += 1
            return self.unary()
        return self.atom()

    def atom(self):
        tok = self.peek()
        if tok == '(':
            self.p += 1
            v = self.expr()
            if self.peek() == ')':
                self.p += 1
            return v
        if isinstance(tok, int):
            self.p += 1
            return tok
        raise ValueError("bad token")


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        print(Parser(tokenize(line)).expr())
    except DivZero:
        print("error")
