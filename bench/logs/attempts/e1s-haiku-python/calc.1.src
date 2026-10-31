import sys


class DivZero(Exception):
    pass


def tokenize(s):
    toks = []
    i = 0
    n = len(s)
    while i < n:
        c = s[i]
        if c.isspace():
            i += 1
        elif c.isdigit():
            j = i
            while j < n and s[j].isdigit():
                j += 1
            toks.append(int(s[i:j]))
            i = j
        elif c in "+-*/()":
            toks.append(c)
            i += 1
        else:
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
        while self.peek() in ("+", "-"):
            op = self.next()
            r = self.term()
            v = v + r if op == "+" else v - r
        return v

    def term(self):
        v = self.unary()
        while self.peek() in ("*", "/"):
            op = self.next()
            r = self.unary()
            if op == "*":
                v = v * r
            else:
                v = trunc_div(v, r)
        return v

    def unary(self):
        if self.peek() == "-":
            self.next()
            return -self.unary()
        if self.peek() == "+":
            self.next()
            return self.unary()
        return self.primary()

    def primary(self):
        t = self.next()
        if t == "(":
            v = self.expr()
            if self.peek() == ")":
                self.next()
            return v
        if isinstance(t, int):
            return t
        raise ValueError("bad token")


def evaluate(line):
    toks = tokenize(line)
    if not toks:
        return None
    p = Parser(toks)
    return p.expr()


def main():
    out = []
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            v = evaluate(line)
            out.append(str(v))
        except DivZero:
            out.append("error")
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
