import sys
import csv


def parse_cents(s):
    s = s.strip()
    neg = s.startswith("-")
    if neg:
        s = s[1:]
    if "." in s:
        whole, frac = s.split(".", 1)
    else:
        whole, frac = s, ""
    frac = (frac + "00")[:2]
    v = int(whole or "0") * 100 + int(frac)
    return -v if neg else v


def main():
    reader = csv.reader(sys.stdin)
    header = next(reader, None)
    totals = {}
    for row in reader:
        if not row or all(not c.strip() for c in row):
            continue
        if len(row) < 3:
            continue
        item = row[0]
        qty = int(row[1].strip())
        price = parse_cents(row[2])
        totals[item] = totals.get(item, 0) + qty * price

    items = sorted(totals.items(), key=lambda kv: (-kv[1], kv[0]))
    lines = []
    for item, cents in items:
        sign = "-" if cents < 0 else ""
        a = abs(cents)
        lines.append("%s: %s%d.%02d" % (item, sign, a // 100, a % 100))
    if lines:
        print("\n".join(lines))


main()
