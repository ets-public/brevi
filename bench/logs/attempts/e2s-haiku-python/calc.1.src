import re
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
            toks.append(("num", int(s[i:j])))
            i = j
        elif c in "+-*/()":
            toks.append((c, c))
            i += 1
        else:
            raise ValueError("bad char")
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
        return self.toks[self.pos][0] if self.pos < len(self.toks) else None

    def expr(self):
        v = self.term()
        while self.peek() in ("+", "-"):
            op = self.toks[self.pos][0]
            self.pos += 1
            r = self.term()
            v = v + r if op == "+" else v - r
        return v

    def term(self):
        v = self.factor()
        while self.peek() in ("*", "/"):
            op = self.toks[self.pos][0]
            self.pos += 1
            r = self.factor()
            v = v * r if op == "*" else trunc_div(v, r)
        return v

    def factor(self):
        p = self.peek()
        if p == "-":
            self.pos += 1
            return -self.factor()
        if p == "+":
            self.pos += 1
            return self.factor()
        if p == "(":
            self.pos += 1
            v = self.expr()
            if self.peek() != ")":
                raise ValueError("expected )")
            self.pos += 1
            return v
        if p == "num":
            v = self.toks[self.pos][1]
            self.pos += 1
            return v
        raise ValueError("unexpected")


def main():
    out = []
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            p = Parser(tokenize(line))
            v = p.expr()
            if p.pos != len(p.toks):
                raise ValueError("trailing")
            out.append(str(v))
        except DivZero:
            out.append("error")
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
