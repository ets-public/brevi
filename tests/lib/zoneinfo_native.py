# zoneinfo, natively: Brevi reads the system's tz database (TZif files and their POSIX TZ rules) as CPython's
# zoneinfo does: offsets, DST, names, PEP 495 fold for repeated and skipped times, and its errors.
from datetime import datetime, timedelta, timezone
from importlib.metadata import PackageNotFoundError, version
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

UTC = timezone.utc
zones = ["America/New_York", "Europe/London", "Australia/Lord_Howe", "Asia/Kolkata", "Pacific/Chatham", "America/Sao_Paulo"]
for key in zones:
    z = ZoneInfo(key)
    print(repr(z), str(z), z.key, ZoneInfo(key) is z)
    for y, m, d, h in [(1970, 1, 1, 0), (2001, 3, 25, 1), (2021, 7, 1, 12), (2024, 11, 3, 6), (2049, 10, 31, 23)]:
        t = datetime(y, m, d, h, 30, tzinfo=UTC).astimezone(z)
        print(" ", t.isoformat(), t.tzname(), t.utcoffset(), t.dst(), t.fold)

# a repeated hour (fall back) and a skipped one (spring forward) in New York
ny = ZoneInfo("America/New_York")
for fold in (0, 1):
    t = datetime(2024, 11, 3, 1, 30, tzinfo=ny, fold=fold)
    print(repr(t), t.utcoffset(), t.tzname(), t.astimezone(UTC).isoformat())
gap = datetime(2024, 3, 10, 2, 30, tzinfo=ny)
print(gap.isoformat(), gap.astimezone(UTC).isoformat(), gap.replace(fold=1).astimezone(UTC).isoformat())
a = datetime(2024, 11, 3, 1, 30, tzinfo=ny)
print(a == a.replace(fold=1), a.replace(fold=1) - a, (a + timedelta(hours=1)).isoformat())
print(datetime(2024, 11, 3, 5, 30, tzinfo=UTC).astimezone(ny).fold, datetime(2024, 11, 3, 6, 30, tzinfo=UTC).astimezone(ny).fold)

# conversions between zones
t = datetime(2026, 6, 1, 9, 15, tzinfo=ZoneInfo("Europe/London"))
for key in ["Asia/Tokyo", "America/Los_Angeles", "Pacific/Chatham", "UTC"]:
    u = t.astimezone(ZoneInfo(key))
    print(key, u.isoformat(), u.strftime("%A %H:%M %Z"), u - t)

for bad in ["Mars/Olympus_Mons", "", "../etc/passwd", "/usr/share/zoneinfo/UTC", "Europe/"]:
    try:
        ZoneInfo(bad)
    except ZoneInfoNotFoundError as e:
        print("not found:", e, isinstance(e, KeyError))
    except ValueError as e:
        print("ValueError:", e)

try:
    version("no-such-distribution-brevi")
except PackageNotFoundError as e:
    print("PackageNotFoundError:", e, isinstance(e, ModuleNotFoundError))
