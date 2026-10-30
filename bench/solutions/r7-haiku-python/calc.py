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
        elif c in '+-*/()':
            toks.append(c)
            i += 1
        else:
            raise ValueError("bad char")
    return toks

def evaluate(s):
    toks = tokenize(s)
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None

    def take():
        t = toks[pos[0]]
        pos[0] += 1
        return t

    def expr():
        v = term()
        while peek() in ('+', '-'):
            op = take()
            r = term()
            v = v + r if op == '+' else v - r
        return v

    def term():
        v = factor()
        while peek() in ('*', '/'):
            op = take()
            r = factor()
            if op == '*':
                v = v * r
            else:
                if r == 0:
                    raise DivZero()
                q = abs(v) // abs(r)
                v = q if (v < 0) == (r < 0) else -q
        return v

    def factor():
        t = peek()
        if t == '-':
            take()
            return -factor()
        if t == '+':
            take()
            return factor()
        if t == '(':
            take()
            v = expr()
            take()  # ')'
            return v
        return take()

    return expr()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        print(evaluate(line))
    except DivZero:
        print("error")
