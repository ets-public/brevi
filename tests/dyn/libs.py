from decimal import Decimal, ROUND_HALF_UP, getcontext
from fractions import Fraction
from datetime import date, datetime, timedelta
import csv, sys, statistics, html
a = Decimal("10.25"); b = Decimal("3")
print(a + b, a * b, a / b, a // b, a % b, -a, a.quantize(Decimal("0.1"), rounding=ROUND_HALF_UP), round(a, 1), Decimal(0.1))
print(f"{a:,.2f}|{a:>10}|{Decimal('1234567.891'):,.2f}", sum([a, b, Decimal('0.01')]), Decimal('1.10') + Decimal('2.20'), Decimal(1) / Decimal(7))
print(repr(a), str(Decimal('1E+3')), Decimal('0.000001'), Decimal('0.0000001'), int(a), float(a), a > 10, a == Decimal('10.250'), hash(Decimal('2.0')) == hash(2))
print(Fraction(3, 4) + Fraction(1, 4), Fraction('1.5'), Fraction(1, 3) * 3, repr(Fraction(2, 6)), float(Fraction(1, 3)), Fraction(7, 2) // 1)
d = date(2024, 2, 28)
print(d + timedelta(days=2), (date(2024, 3, 1) - d).days, d.weekday(), d.strftime("%A %d %B %Y"), repr(d), d.isoformat(), date.fromisoformat("2023-12-31").toordinal())
dt = datetime(2024, 1, 5, 10, 30)
print(dt, repr(dt), dt + timedelta(hours=20), datetime.strptime("2024-03-04 05:06", "%Y-%m-%d %H:%M"), timedelta(days=1, seconds=3661), timedelta(seconds=-1))
for row in csv.reader(['a,b,"c,d"\n', '1,"x""y",3\n']):
    print(row)
w = csv.writer(sys.stdout, lineterminator="\n")
w.writerow(["a", "b,c", 3, None, 1.5])
print(statistics.mean([1, 2, 3, 4]), statistics.mean([1, 2]), statistics.median([3, 1, 2, 4]), statistics.mode([1, 2, 2]), statistics.stdev([1, 2, 3, 4]), statistics.variance([1, 3, 5]))
print(html.escape("<a href='x'>&</a>"), getcontext().prec)
getcontext().prec = 5
print(Decimal(1) / Decimal(3))
