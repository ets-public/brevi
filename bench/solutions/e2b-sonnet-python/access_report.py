import sys, re
from collections import Counter

rx = re.compile(r'^(\S+) - - \[([^\]]+)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')
req = mal = tot = 0
ips = set()
st = Counter()
paths = Counter()
hours = Counter()
bots = 0
errs = Counter()
for line in sys.stdin.read().split("\n"):
    line = line.rstrip("\r")
    if not line.strip():
        continue
    m = rx.match(line)
    if not m:
        mal += 1
        continue
    ip, ts, meth, path, proto, status, size, ref, ua = m.groups()
    hm = re.match(r'^[^:]+:(\d{2}):', ts)
    if not hm:
        mal += 1
        continue
    req += 1
    tot += 0 if size == '-' else int(size)
    ips.add(ip)
    st[status[0] + "xx"] += 1
    paths[path.split("?")[0]] += 1
    hours[hm.group(1)] += 1
    if "bot" in ua.lower():
        bots += 1
    if int(status) >= 400:
        errs[ip] += 1
print("requests %d" % req)
print("malformed %d" % mal)
print("bytes %d" % tot)
print("unique_ips %d" % len(ips))
print("status")
for c in ("2xx", "3xx", "4xx", "5xx"):
    if c in st:
        print("  %s %d" % (c, st[c]))
print("top_paths")
for p, n in sorted(paths.items(), key=lambda x: (-x[1], x[0]))[:3]:
    print("  %d %s" % (n, p))
print("hours")
if hours:
    mx = max(hours.values())
    for h in sorted(hours):
        n = hours[h]
        k = max(1, (n * 40 + mx) // (2 * mx))
        print("  %s %d %s" % (h, n, "#" * k))
print("bots %d" % bots)
print("errors_by_ip")
for ip, n in sorted(errs.items(), key=lambda x: (-x[1], x[0])):
    if n >= 2:
        print("  %s %d" % (ip, n))
