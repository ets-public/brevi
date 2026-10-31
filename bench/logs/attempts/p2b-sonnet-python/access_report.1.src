import sys, re
from collections import Counter
rx = re.compile(r'^(\S+) - - \[(\d+/\w+/\d+:(\d\d):\d\d:\d\d [^\]]*)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')
req = mal = total = bots = 0
ips = set(); st = Counter(); paths = Counter(); hrs = Counter(); err = Counter()
for line in sys.stdin.read().split('\n'):
    line = line.rstrip('\r')
    if not line.strip():
        continue
    m = rx.match(line)
    if not m:
        mal += 1
        continue
    req += 1
    ip, hh, path, status, size, ua = m.group(1), m.group(3), m.group(5), int(m.group(7)), m.group(8), m.group(10)
    path = path.split('?')[0]
    total += 0 if size == '-' else int(size)
    ips.add(ip)
    st[status // 100] += 1
    paths[path] += 1
    hrs[hh] += 1
    if 'bot' in ua.lower():
        bots += 1
    if status >= 400:
        err[ip] += 1
print("requests", req)
print("malformed", mal)
print("bytes", total)
print("unique_ips", len(ips))
print("status")
for c in (2, 3, 4, 5):
    if c in st:
        print("  %dxx %d" % (c, st[c]))
print("top_paths")
for p, n in sorted(paths.items(), key=lambda x: (-x[1], x[0]))[:3]:
    print("  %d %s" % (n, p))
print("hours")
if hrs:
    mx = max(hrs.values())
    for h in sorted(hrs):
        n = hrs[h]
        b = max(1, (n * 40 + mx) // (2 * mx))
        print("  %s %d %s" % (h, n, '#' * b))
print("bots", bots)
print("errors_by_ip")
for ip, n in sorted(err.items(), key=lambda x: (-x[1], x[0])):
    if n >= 2:
        print("  %s %d" % (ip, n))
