import re
import sys

VAL = {'I': 1, 'V': 5, 'X': 10, 'L': 50, 'C': 100, 'D': 500, 'M': 1000}

def from_roman(s):
    total = 0
    for i, ch in enumerate(s):
        v = VAL[ch]
        if i + 1 < len(s) and v < VAL[s[i + 1]]:
            total -= v
        else:
            total += v
    return total

def to_roman(n):
    table = [(1000, 'M'), (900, 'CM'), (500, 'D'), (400, 'CD'), (100, 'C'), (90, 'XC'),
             (50, 'L'), (40, 'XL'), (10, 'X'), (9, 'IX'), (5, 'V'), (4, 'IV'), (1, 'I')]
    out = []
    for v, sym in table:
        while n >= v:
            out.append(sym)
            n -= v
    return ''.join(out)

for line in sys.stdin:
    tokens = re.findall(r'[IVXLCDM]+|[+-]', line)
    if not tokens:
        continue
    result = from_roman(tokens[0])
    i = 1
    while i + 1 < len(tokens):
        op, num = tokens[i], from_roman(tokens[i + 1])
        result = result + num if op == '+' else result - num
        i += 2
    print('nulla' if result <= 0 else to_roman(result))
