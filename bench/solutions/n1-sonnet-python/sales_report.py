import sys, re, csv
from collections import defaultdict
lines = sys.stdin.read().split('\n')
lines = [l.rstrip('\r') for l in lines]
rows = skipped = 0
reg = defaultdict(int); qtys = defaultdict(int); mon = defaultdict(int)
def cents(p):
    if '.' in p:
        a, b = p.split('.')
        return int(a) * 100 + int(b.ljust(2, '0'))
    return int(p) * 100
first = True
for l in lines:
    if l.strip() == '':
        continue
    if first:
        first = False
        continue
    try:
        f = next(csv.reader([l]))
    except Exception:
        skipped += 1; continue
    if (len(f) != 5 or not re.fullmatch(r'\d{4}-\d{2}-\d{2}', f[0]) or not re.fullmatch(r'[0-9]+', f[3])
            or not re.fullmatch(r'[0-9]+(\.[0-9]{1,2})?', f[4])):
        skipped += 1; continue
    q = int(f[3]); r = q * cents(f[4])
    rows += 1
    reg[f[1]] += r; qtys[f[2]] += q; mon[f[0][:7]] += r
def fmt(c):
    return "%d.%02d" % (c // 100, c % 100)
print("rows", rows); print("skipped", skipped)
if rows:
    print("regions")
    for k, v in sorted(reg.items(), key=lambda x: (-x[1], x[0])):
        print("  %s %s" % (k, fmt(v)))
    n, q = sorted(qtys.items(), key=lambda x: (-x[1], x[0]))[0]
    print("top_product %s %d" % (n, q))
    print("months")
    for k in sorted(mon):
        print("  %s %s" % (k, fmt(mon[k])))
