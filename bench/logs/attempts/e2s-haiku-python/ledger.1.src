import csv
import sys
from decimal import Decimal


def main():
    reader = csv.reader(sys.stdin)
    header = None
    months = {}
    balances = {}
    for row in reader:
        if not row or all(not c.strip() for c in row):
            continue
        if header is None:
            header = [h.strip() for h in row]
            continue
        rec = dict(zip(header, [c.strip() for c in row]))
        date = rec["date"]
        account = rec["account"]
        amt = Decimal(rec["amount"])
        month = date[:7]
        inc, outc = months.get(month, (Decimal("0"), Decimal("0")))
        if amt >= 0:
            inc += amt
        else:
            outc += -amt
        months[month] = (inc, outc)
        balances[account] = balances.get(account, Decimal("0")) + amt

    out = []
    for month in sorted(months):
        inc, outc = months[month]
        out.append(f"{month} {inc:.2f} {outc:.2f} {(inc - outc):.2f}")
    if balances:
        acct, bal = min(balances.items(), key=lambda kv: (-kv[1], kv[0]))
        out.append(f"top {acct} {bal:.2f}")
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
