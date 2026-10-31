import sys

def tokenize(s):
    t = []
    i = 0
    while i < len(s):
        c = s[i]
        if c.isspace():
            i += 1
        elif c.isdigit():
            j = i
            while j < len(s) and s[j].isdigit():
                j += 1
            t.append(int(s[i:j]))
            i = j
        else:
            t.append(c)
            i += 1
    return t

class DZ(Exception):
    pass

def evaluate(s):
    t = tokenize(s)
    pos = [0]

    def peek():
        return t[pos[0]] if pos[0] < len(t) else None

    def expr():
        v = term()
        while peek() in ('+', '-'):
            op = t[pos[0]]
            pos[0] += 1
            r = term()
            v = v + r if op == '+' else v - r
        return v

    def term():
        v = unary()
        while peek() in ('*', '/'):
            op = t[pos[0]]
            pos[0] += 1
            r = unary()
            if op == '*':
                v = v * r
            else:
                if r == 0:
                    raise DZ()
                q = abs(v) // abs(r)
                v = q if (v >= 0) == (r >= 0) else -q
        return v

    def unary():
        p = peek()
        if p == '-':
            pos[0] += 1
            return -unary()
        if p == '+':
            pos[0] += 1
            return unary()
        if p == '(':
            pos[0] += 1
            v = expr()
            pos[0] += 1
            return v
        pos[0] += 1
        return p

    return expr()

for line in sys.stdin:
    if not line.strip():
        continue
    try:
        print(evaluate(line))
    except DZ:
        print("error")
