import sys
from decimal import Decimal, ROUND_HALF_UP


def q1(x):
    return x.quantize(Decimal("0.1"), rounding=ROUND_HALF_UP)


def main():
    paths = {}
    for line in sys.stdin.read().splitlines():
        parts = line.split()
        if len(parts) < 4:
            continue
        method, path, status, ms = parts[0], parts[1], int(parts[2]), Decimal(parts[3])
        paths.setdefault(path, []).append((status, ms))

    out = []
    for path in sorted(paths):
        recs = paths[path]
        n = len(recs)
        times = sorted(ms for _, ms in recs)
        total = sum(times, Decimal("0"))
        avg = q1(total / n)
        k = (9 * n + 9) // 10  # ceil(0.9 * n)
        p90 = times[k - 1]
        errs = sum(1 for s, _ in recs if s >= 500)
        err = q1(Decimal(errs) * 100 / n)
        p90_s = str(int(p90)) if p90 == p90.to_integral_value() else str(p90.normalize())
        out.append(f"{path} {n} {avg:.1f} {p90_s} {err:.1f}%")
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
