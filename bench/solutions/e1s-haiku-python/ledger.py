import sys
import csv


def parse_cents(s):
    s = s.strip()
    neg = s.startswith("-")
    if neg or s.startswith("+"):
        s = s[1:]
    if "." in s:
        whole, frac = s.split(".", 1)
    else:
        whole, frac = s, ""
    frac = (frac + "00")[:2]
    v = int(whole or "0") * 100 + int(frac)
    return -v if neg else v


def fmt(cents):
    sign = "-" if cents < 0 else ""
    a = abs(cents)
    return "%s%d.%02d" % (sign, a // 100, a % 100)


def main():
    reader = csv.reader(sys.stdin)
    header = next(reader, None)
    months = {}
    balances = {}
    for row in reader:
        if len(row) < 3:
            continue
        date = row[0].strip()
        acct = row[1].strip()
        amt = parse_cents(row[2])
        m = date[:7]
        inc, outc, net = months.get(m, (0, 0, 0))
        if amt > 0:
            inc += amt
        elif amt < 0:
            outc += -amt
        months[m] = (inc, outc, net + amt)
        balances[acct] = balances.get(acct, 0) + amt

    out = []
    for m in sorted(months):
        inc, outc, net = months[m]
        out.append("%s %s %s %s" % (m, fmt(inc), fmt(outc), fmt(inc - outc)))
    if balances:
        best = None
        for acct, bal in balances.items():
            if best is None or bal > balances[best] or (bal == balances[best] and acct < best):
                best = acct
        out.append("top %s %s" % (best, fmt(balances[best])))
    if out:
        print("\n".join(out))


main()
