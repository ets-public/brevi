import sys, re
from collections import Counter

pat = re.compile(r'^(\S+) - - \[([^\]]*)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')
req = mal = total = bots = 0
ips = set()
status = Counter()
paths = Counter()
hours = Counter()
errs = Counter()
for line in sys.stdin.read().split("\n"):
    line = line.rstrip("\r")
    if line.strip() == "":
        continue
    m = pat.match(line)
    if not m:
        mal += 1
        continue
    ip, ts, meth, path, proto, st, size, ref, ua = m.groups()
    tm = re.match(r'^[^:]*:(\d{2}):', ts)
    if not tm:
        mal += 1
        continue
    req += 1
    total += 0 if size == "-" else int(size)
    ips.add(ip)
    st = int(st)
    status[st // 100] += 1
    paths[path.split("?")[0]] += 1
    hours[tm.group(1)] += 1
    if "bot" in ua.lower():
        bots += 1
    if st >= 400:
        errs[ip] += 1
print("requests %d" % req)
print("malformed %d" % mal)
print("bytes %d" % total)
print("unique_ips %d" % len(ips))
print("status")
for c in sorted(status):
    print("  %dxx %d" % (c, status[c]))
print("top_paths")
for p, n in sorted(paths.items(), key=lambda x: (-x[1], x[0]))[:3]:
    print("  %d %s" % (n, p))
print("hours")
if hours:
    mx = max(hours.values())
    for h in sorted(hours):
        n = hours[h]
        k = (n * 40 + mx) // (2 * mx)
        print("  %s %d %s" % (h, n, "#" * max(1, k)))
print("bots %d" % bots)
print("errors_by_ip")
for ip, n in sorted(errs.items(), key=lambda x: (-x[1], x[0])):
    if n >= 2:
        print("  %s %d" % (ip, n))
