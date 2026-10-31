import sys

class DZ(Exception):
    pass

def evaluate(s):
    pos = [0]
    def peek():
        while pos[0] < len(s) and s[pos[0]] == ' ':
            pos[0] += 1
        return s[pos[0]] if pos[0] < len(s) else ''
    def expr():
        v = term()
        while peek() in ('+', '-'):
            op = s[pos[0]]; pos[0] += 1
            r = term()
            v = v + r if op == '+' else v - r
        return v
    def term():
        v = unary()
        while peek() in ('*', '/'):
            op = s[pos[0]]; pos[0] += 1
            r = unary()
            if op == '*':
                v = v * r
            else:
                if r == 0:
                    raise DZ()
                q = abs(v) // abs(r)
                v = q if (v < 0) == (r < 0) else -q
        return v
    def unary():
        c = peek()
        if c == '-':
            pos[0] += 1
            return -unary()
        if c == '+':
            pos[0] += 1
            return unary()
        if c == '(':
            pos[0] += 1
            v = expr()
            peek()
            pos[0] += 1
            return v
        st = pos[0]
        while pos[0] < len(s) and s[pos[0]].isdigit():
            pos[0] += 1
        return int(s[st:pos[0]])
    return expr()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        print(evaluate(line))
    except DZ:
        print("error")
