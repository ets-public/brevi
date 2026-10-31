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
            continue
        if c.isdigit():
            j = i
            while j < len(s) and s[j].isdigit():
                j += 1
            toks.append(int(s[i:j]))
            i = j
            continue
        toks.append(c)
        i += 1
    return toks


def tdiv(a, b):
    if b == 0:
        raise DivZero()
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b > 0) else -q


def evaluate(s):
    toks = tokenize(s)
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None

    def eat():
        t = toks[pos[0]]
        pos[0] += 1
        return t

    def expr():
        v = term()
        while peek() in ('+', '-'):
            op = eat()
            r = term()
            v = v + r if op == '+' else v - r
        return v

    def term():
        v = factor()
        while peek() in ('*', '/'):
            op = eat()
            r = factor()
            v = v * r if op == '*' else tdiv(v, r)
        return v

    def factor():
        t = peek()
        if t == '-':
            eat()
            return -factor()
        if t == '+':
            eat()
            return factor()
        if t == '(':
            eat()
            v = expr()
            eat()
            return v
        eat()
        return t

    return expr()


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        print(evaluate(line))
    except DivZero:
        print("error")
