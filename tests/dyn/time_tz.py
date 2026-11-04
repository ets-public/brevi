# time.gmtime/strftime and datetime.timezone in dynamic builds (#194).
import time
import datetime as dt
from datetime import datetime, timezone, timedelta, date

class P:
    def __init__(self, d):
        self.data = d
    def __getitem__(self, k):
        return self.data[k]
    def get(self, k, default=None):
        try:
            return self[k]
        except KeyError:
            return default

print(P({}).get("a"))
g = time.gmtime(86400 * 365.7)
print(g)
print(g.tm_year, g[0], tuple(g)[:6], len(g))
print(time.strftime("%Y-%m-%dT%H:%M:%SZ %z %Z %j %a", g))
print(time.strftime("%Y-%m-%d", time.gmtime(-1.5)))
print(time.strftime("%Y", (2020, 1, 2, 3, 4, 5, 0, 1, 0)))
u = timezone.utc
print(repr(u), str(u), u == dt.timezone(timedelta(0)))
e = timezone(timedelta(hours=-5, minutes=-30))
print(repr(e), str(e), e.utcoffset(None))
d = datetime(2024, 3, 10, 12, 30, tzinfo=u)
print(repr(d), str(d), d.isoformat(), d.timestamp())
print(d.astimezone(e), d.astimezone(e) == d, hash(d.astimezone(e)) == hash(d))
print(d.strftime("%z %Z"), d.tzinfo, d.utcoffset(), d.tzname())
print(dt.datetime.combine(date(2024, 1, 1), dt.time(), dt.timezone.utc).timestamp())
print(datetime.fromisoformat("2024-01-01T10:00:00+02:00"), datetime.fromisoformat("2024-01-01T10:00:00Z").tzinfo)
print(d - datetime(2024, 3, 10, 0, 0, tzinfo=e), d + timedelta(days=1))
print(d.replace(tzinfo=None), d.replace(hour=1))
try:
    print(d - datetime(2024, 1, 1))
except TypeError as x:
    print("TypeError", x)
print(d == datetime(2024, 3, 10, 12, 30))
print(time.time_ns() > 0, dt.UTC, datetime.now(timezone.utc).tzinfo)
