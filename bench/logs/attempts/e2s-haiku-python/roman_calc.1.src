import re
import sys

VALUES = {"I": 1, "V": 5, "X": 10, "L": 50, "C": 100, "D": 500, "M": 1000}
NUMERALS = [
    (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"),
    (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
    (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
]


def roman_to_int(s):
    total = 0
    for i, ch in enumerate(s):
        v = VALUES[ch]
        if i + 1 < len(s) and v < VALUES[s[i + 1]]:
            total -= v
        else:
            total += v
    return total


def int_to_roman(n):
    out = []
    for v, sym in NUMERALS:
        while n >= v:
            out.append(sym)
            n -= v
    return "".join(out)


def main():
    out = []
    for line in sys.stdin.read().splitlines():
        tokens = re.findall(r"[IVXLCDM]+|[+\-]", line)
        if not tokens:
            continue
        result = roman_to_int(tokens[0])
        i = 1
        while i + 1 < len(tokens):
            op = tokens[i]
            num = roman_to_int(tokens[i + 1])
            result = result + num if op == "+" else result - num
            i += 2
        out.append("nulla" if result <= 0 else int_to_roman(result))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
