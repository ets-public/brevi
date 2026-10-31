import sys

VAL = {"I": 1, "V": 5, "X": 10, "L": 50, "C": 100, "D": 500, "M": 1000}
SYMS = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"),
        (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")]


def roman_value(s):
    total = 0
    for i, ch in enumerate(s):
        v = VAL[ch]
        if i + 1 < len(s) and VAL[s[i + 1]] > v:
            total -= v
        else:
            total += v
    return total


def to_roman(n):
    out = []
    for v, sym in SYMS:
        while n >= v:
            out.append(sym)
            n -= v
    return "".join(out)


for line in sys.stdin:
    parts = line.split()
    if not parts:
        continue
    result = roman_value(parts[0])
    op = None
    for tok in parts[1:]:
        if tok in ("+", "-"):
            op = tok
        else:
            v = roman_value(tok)
            result = result + v if op == "+" else result - v
    if result <= 0:
        print("nulla")
    else:
        print(to_roman(result))
