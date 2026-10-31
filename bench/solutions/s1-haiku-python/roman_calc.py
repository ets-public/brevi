import sys

VALUES = {'I': 1, 'V': 5, 'X': 10, 'L': 50, 'C': 100, 'D': 500, 'M': 1000}


def from_roman(s):
    total = 0
    for i, c in enumerate(s):
        v = VALUES[c]
        if i + 1 < len(s) and VALUES[s[i + 1]] > v:
            total -= v
        else:
            total += v
    return total


def to_roman(n):
    table = [
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"),
        (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
        (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ]
    out = []
    for v, sym in table:
        while n >= v:
            out.append(sym)
            n -= v
    return "".join(out)


for line in sys.stdin:
    toks = line.split()
    if not toks:
        continue
    result = from_roman(toks[0])
    i = 1
    while i + 1 < len(toks):
        op = toks[i]
        val = from_roman(toks[i + 1])
        result = result + val if op == '+' else result - val
        i += 2
    if result <= 0:
        print("nulla")
    else:
        print(to_roman(result))
