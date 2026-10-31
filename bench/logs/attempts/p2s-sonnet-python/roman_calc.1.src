import sys
V = {'I': 1, 'V': 5, 'X': 10, 'L': 50, 'C': 100, 'D': 500, 'M': 1000}

def parse(s):
    t = 0
    for i, c in enumerate(s):
        v = V[c]
        if i + 1 < len(s) and V[s[i + 1]] > v:
            t -= v
        else:
            t += v
    return t

def to_roman(n):
    out = ""
    for v, s in ((1000,'M'),(900,'CM'),(500,'D'),(400,'CD'),(100,'C'),(90,'XC'),(50,'L'),(40,'XL'),(10,'X'),(9,'IX'),(5,'V'),(4,'IV'),(1,'I')):
        while n >= v:
            out += s
            n -= v
    return out

for line in sys.stdin:
    p = line.split()
    if not p:
        continue
    r = parse(p[0])
    for i in range(1, len(p) - 1, 2):
        if p[i] == '+':
            r += parse(p[i + 1])
        else:
            r -= parse(p[i + 1])
    print(to_roman(r) if r > 0 else "nulla")
