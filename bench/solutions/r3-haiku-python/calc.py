import sys


class DivByZero(Exception):
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
        elif c in "+-*/()":
            tokens.append(c)
            i += 1
        else:
            raise ValueError("bad char")
    return tokens


def trunc_div(a, b):
    if b == 0:
        raise DivByZero()
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
        while self.peek() in ("+", "-"):
            op = self.t[self.p]
            self.p += 1
            r = self.term()
            v = v + r if op == "+" else v - r
        return v

    def term(self):
        v = self.unary()
        while self.peek() in ("*", "/"):
            op = self.t[self.p]
            self.p += 1
            r = self.unary()
            if op == "*":
                v = v * r
            else:
                v = trunc_div(v, r)
        return v

    def unary(self):
        if self.peek() == "-":
            self.p += 1
            return -self.unary()
        if self.peek() == "+":
            self.p += 1
            return self.unary()
        return self.primary()

    def primary(self):
        tok = self.peek()
        if isinstance(tok, int):
            self.p += 1
            return tok
        if tok == "(":
            self.p += 1
            v = self.expr()
            if self.peek() == ")":
                self.p += 1
            return v
        raise ValueError("unexpected")


def main():
    out = []
    for line in sys.stdin.read().split("\n"):
        if not line.strip():
            continue
        try:
            toks = tokenize(line)
            p = Parser(toks)
            val = p.expr()
            out.append(str(val))
        except DivByZero:
            out.append("error")
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
