# isoformat(timespec=...) and time.fromisoformat (#211).
import datetime as dt

b = dt.datetime(2026, 1, 2, 3, 4, 5, 123456, tzinfo=dt.timezone.utc)
print(b.isoformat(timespec="seconds"))
for s in ["hours", "minutes", "seconds", "milliseconds", "microseconds", "auto"]:
    print(b.isoformat(" ", s), dt.datetime(2026, 1, 2).isoformat(timespec=s), dt.time(1, 2, 3, 4500).isoformat(s), dt.time(1, 2).isoformat(timespec=s))
try:
    b.isoformat(timespec="x")
except ValueError as e:
    print("ValueError", e)
print(dt.time.fromisoformat("19:05"), dt.date.fromisoformat("2026-11-09"))
print(dt.time.fromisoformat("190500"), dt.time.fromisoformat("1905"), dt.time.fromisoformat("19:05:01.5"), repr(dt.time.fromisoformat("T07")))
print(dt.time.min, dt.time.max, dt.time.resolution)
print(dt.datetime.fromisoformat("2026-01-02T030405").isoformat())
try:
    dt.time.fromisoformat("19:5")
except ValueError as e:
    print("ValueError", e)
