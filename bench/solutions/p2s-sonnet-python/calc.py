import sys

class DZ(Exception):
    pass

def evaluate(s):
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
    pos = [0]

    def peek():
        return toks[pos[0]] if pos[0] < len(toks) else None

    def expr():
        v = term()
        while peek() in ('+', '-'):
            op = toks[pos[0]]
            pos[0] += 1
            r = term()
            v = v + r if op == '+' else v - r
        return v

    def term():
        v = unary()
        while peek() in ('*', '/'):
            op = toks[pos[0]]
            pos[0] += 1
            r = unary()
            if op == '*':
                v *= r
            else:
                if r == 0:
                    raise DZ()
                q = abs(v) // abs(r)
                v = q if (v >= 0) == (r >= 0) else -q
        return v

    def unary():
        t = peek()
        if t == '-':
            pos[0] += 1
            return -unary()
        if t == '+':
            pos[0] += 1
            return unary()
        if t == '(':
            pos[0] += 1
            v = expr()
            pos[0] += 1
            return v
        pos[0] += 1
        return t

    return expr()

for line in sys.stdin:
    if not line.strip():
        continue
    try:
        print(evaluate(line))
    except DZ:
        print("error")
