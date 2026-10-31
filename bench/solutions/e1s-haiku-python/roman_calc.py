import sys

VAL = {"I": 1, "V": 5, "X": 10, "L": 50, "C": 100, "D": 500, "M": 1000}


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
    pairs = [
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"),
        (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
        (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ]
    res = []
    for v, sym in pairs:
        while n >= v:
            res.append(sym)
            n -= v
    return "".join(res)


out = []
for line in sys.stdin:
    toks = line.split()
    if not toks:
        continue
    result = from_roman(toks[0])
    i = 1
    while i + 1 < len(toks):
        op = toks[i]
        val = from_roman(toks[i + 1])
        if op == "+":
            result += val
        else:
            result -= val
        i += 2
    if result <= 0:
        out.append("nulla")
    else:
        out.append(to_roman(result))

if out:
    print("\n".join(out))
