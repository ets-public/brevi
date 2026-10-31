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
T = [(1000,'M'),(900,'CM'),(500,'D'),(400,'CD'),(100,'C'),(90,'XC'),(50,'L'),(40,'XL'),(10,'X'),(9,'IX'),(5,'V'),(4,'IV'),(1,'I')]
def fmt(n):
    r = ''
    for v, s in T:
        while n >= v:
            r += s
            n -= v
    return r
for line in sys.stdin:
    p = line.split()
    if not p:
        continue
    t = parse(p[0])
    for i in range(1, len(p) - 1, 2):
        x = parse(p[i + 1])
        t = t + x if p[i] == '+' else t - x
    print(fmt(t) if t > 0 else "nulla")
