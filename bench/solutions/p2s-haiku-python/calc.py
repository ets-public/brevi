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


def parse(toks):
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None

    def advance():
        t = toks[pos[0]]
        pos[0] += 1
        return t

    def expr():
        v = term()
        while peek() in ('+', '-'):
            op = advance()
            r = term()
            v = v + r if op == '+' else v - r
        return v

    def term():
        v = unary()
        while peek() in ('*', '/'):
            op = advance()
            r = unary()
            v = v * r if op == '*' else tdiv(v, r)
        return v

    def unary():
        if peek() == '-':
            advance()
            return -unary()
        if peek() == '+':
            advance()
            return unary()
        return atom()

    def atom():
        t = advance()
        if t == '(':
            v = expr()
            advance()  # ')'
            return v
        return t

    return expr()


def main():
    for line in sys.stdin:
        s = line.strip()
        if not s:
            continue
        try:
            toks = tokenize(s)
            print(parse(toks))
        except DivZero:
            print("error")


main()
