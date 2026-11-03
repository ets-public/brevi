from datetime import datetime, time, timedelta
from decimal import Decimal

now = datetime(2026, 11, 3, 7, 34, 5)
t = now.time()
print(t, repr(t), t.hour, t.minute, t.second)
print(datetime.combine(now.date(), t) == now, datetime.combine(now.date() + timedelta(days=1), time(9, 30)))
print(time(9, 30) < time(10), time(23, 59, 59, 5), time().isoformat(), t.strftime("%H:%M"))
print(f"{t:%H.%M}", t.replace(hour=1), {time(1): "a"}[time(1, 0)])
print(Decimal(1).scaleb(-2), Decimal("3.14159").quantize(Decimal(1).scaleb(-3)), Decimal("2.5").scaleb(3))
