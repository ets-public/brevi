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


def tdiv(a, b):
    if b == 0:
        raise DivZero()
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


class Parser:
    def __init__(self, toks):
        self.t = toks
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
        v = self.factor()
        while self.peek() in ('*', '/'):
            op = self.t[self.p]
            self.p += 1
            r = self.factor()
            v = v * r if op == '*' else tdiv(v, r)
        return v

    def factor(self):
        tok = self.peek()
        if tok == '-':
            self.p += 1
            return -self.factor()
        if tok == '+':
            self.p += 1
            return self.factor()
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


def main():
    out = []
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            p = Parser(tokenize(line))
            v = p.expr()
            out.append(str(v))
        except DivZero:
            out.append("error")
    print("\n".join(out))


main()
