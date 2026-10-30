import sys
from collections import defaultdict
from fractions import Fraction
from decimal import Decimal, ROUND_HALF_UP

def fmt(frac):
    d = Decimal(frac.numerator) / Decimal(frac.denominator)
    return str(d.quantize(Decimal('0.1'), rounding=ROUND_HALF_UP))

def main():
    data = defaultdict(list)
    errs = defaultdict(int)
    for line in sys.stdin:
        parts = line.split()
        if not parts:
            continue
        _method, path, status, ms = parts[0], parts[1], int(parts[2]), int(parts[3])
        data[path].append(ms)
        if status >= 500:
            errs[path] += 1
    out = []
    for path in sorted(data):
        vals = sorted(data[path])
        n = len(vals)
        avg = fmt(Fraction(sum(vals), n))
        rank = (9 * n + 9) // 10
        p90 = vals[rank - 1]
        err = fmt(Fraction(errs[path] * 100, n))
        out.append(f"{path} {n} {avg} {p90} {err}%")
    if out:
        sys.stdout.write('\n'.join(out) + '\n')

main()
