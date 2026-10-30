import sys

VAL = {'I': 1, 'V': 5, 'X': 10, 'L': 50, 'C': 100, 'D': 500, 'M': 1000}
NUMS = [(1000, 'M'), (900, 'CM'), (500, 'D'), (400, 'CD'), (100, 'C'), (90, 'XC'),
        (50, 'L'), (40, 'XL'), (10, 'X'), (9, 'IX'), (5, 'V'), (4, 'IV'), (1, 'I')]

def parse(s):
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
    for v, sym in NUMS:
        while n >= v:
            out.append(sym)
            n -= v
    return ''.join(out)

def main():
    out = []
    for line in sys.stdin:
        tokens = line.split()
        if not tokens:
            continue
        result = parse(tokens[0])
        i = 1
        while i + 1 < len(tokens):
            op, operand = tokens[i], parse(tokens[i + 1])
            result = result + operand if op == '+' else result - operand
            i += 2
        out.append(to_roman(result) if result > 0 else 'nulla')
    if out:
        sys.stdout.write('\n'.join(out) + '\n')

main()
