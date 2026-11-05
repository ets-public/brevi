# A zoneinfo.ZoneInfo as the tzinfo of a datetime the dynamic build makes natively (#211).
import datetime as dt
from zoneinfo import ZoneInfo

ny = ZoneInfo("America/New_York")
at = dt.datetime.combine(dt.date(2026, 11, 9), dt.time(19, 5), ny)
print(at.astimezone(ZoneInfo("Australia/Sydney")).isoformat())
a = dt.datetime(2026, 3, 8, 1, 30, tzinfo=ny)
print(a.isoformat(), (a + dt.timedelta(hours=1)).isoformat(), a.tzname())
b = dt.datetime(2026, 1, 2, 3, 4, 5, 123456, tzinfo=dt.timezone.utc)
print(b.astimezone(ny).isoformat(timespec="minutes"), b.replace(tzinfo=ny).isoformat())
print(dt.datetime.fromtimestamp(0, ny).isoformat(), dt.datetime.fromtimestamp(0, tz=ny).isoformat())
print(a - b, b < a, a.astimezone(dt.timezone.utc) == a)
