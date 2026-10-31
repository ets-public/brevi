import sys

VAL = {'I': 1, 'V': 5, 'X': 10, 'L': 50, 'C': 100, 'D': 500, 'M': 1000}


def from_roman(s):
    total = 0
    for i, c in enumerate(s):
        v = VAL[c]
        if i + 1 < len(s) and VAL[s[i + 1]] > v:
            total -= v
        else:
            total += v
    return total


def to_roman(n):
    table = [(1000, 'M'), (900, 'CM'), (500, 'D'), (400, 'CD'),
             (100, 'C'), (90, 'XC'), (50, 'L'), (40, 'XL'),
             (10, 'X'), (9, 'IX'), (5, 'V'), (4, 'IV'), (1, 'I')]
    out = []
    for v, sym in table:
        while n >= v:
            out.append(sym)
            n -= v
    return ''.join(out)


for line in sys.stdin:
    parts = line.split()
    if not parts:
        continue
    result = from_roman(parts[0])
    i = 1
    while i + 1 < len(parts) + 1 and i < len(parts):
        op = parts[i]
        operand = from_roman(parts[i + 1])
        if op == '+':
            result += operand
        else:
            result -= operand
        i += 2
    if result <= 0:
        print("nulla")
    else:
        print(to_roman(result))
