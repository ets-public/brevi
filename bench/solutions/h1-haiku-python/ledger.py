import sys
import csv
from decimal import Decimal

def cents(s):
    return int((Decimal(s.strip()) * 100).to_integral_value())

def money(c):
    sign = '-' if c < 0 else ''
    c = abs(c)
    return f"{sign}{c // 100}.{c % 100:02d}"

def main():
    reader = csv.reader(sys.stdin)
    header = next(reader, None)
    months = {}
    balances = {}
    for row in reader:
        if not row or all(not x.strip() for x in row):
            continue
        date, account, amount = row[0].strip(), row[1].strip(), cents(row[2])
        month = date[:7]
        inn, outt = months.get(month, (0, 0))
        if amount > 0:
            inn += amount
        elif amount < 0:
            outt += -amount
        months[month] = (inn, outt)
        balances[account] = balances.get(account, 0) + amount
    out = []
    for month in sorted(months):
        inn, outt = months[month]
        out.append(f"{month} {money(inn)} {money(outt)} {money(inn - outt)}")
    if balances:
        best = min(balances, key=lambda a: (-balances[a], a))
        out.append(f"top {best} {money(balances[best])}")
    if out:
        sys.stdout.write('\n'.join(out) + '\n')

main()
