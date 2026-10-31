import sys, re
from collections import Counter

pat = re.compile(r'^(\S+) - - \[(\d+/\w+/\d+):(\d\d):\d\d:\d\d [^\]]*\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')
req = 0
mal = 0
total = 0
ips = set()
classes = Counter()
paths = Counter()
hours = Counter()
bots = 0
errs = Counter()
for line in sys.stdin.read().split("\n"):
    line = line.rstrip("\r")
    if not line.strip():
        continue
    m = pat.match(line)
    if not m:
        mal += 1
        continue
    ip, _, hh, meth, path, proto, st, size, ref, ua = m.groups()
    req += 1
    total += 0 if size == "-" else int(size)
    ips.add(ip)
    classes[st[0] + "xx"] += 1
    paths[path.split("?")[0]] += 1
    hours[hh] += 1
    if "bot" in ua.lower():
        bots += 1
    if int(st) >= 400:
        errs[ip] += 1
out = []
out.append("requests %d" % req)
out.append("malformed %d" % mal)
out.append("bytes %d" % total)
out.append("unique_ips %d" % len(ips))
out.append("status")
for c in sorted(classes):
    out.append("  %s %d" % (c, classes[c]))
out.append("top_paths")
for p, n in sorted(paths.items(), key=lambda x: (-x[1], x[0]))[:3]:
    out.append("  %d %s" % (n, p))
out.append("hours")
if hours:
    mx = max(hours.values())
    for h in sorted(hours):
        n = hours[h]
        b = max(1, (n * 40 + mx) // (2 * mx))
        out.append("  %s %d %s" % (h, n, "#" * b))
out.append("bots %d" % bots)
out.append("errors_by_ip")
for ip, n in sorted(errs.items(), key=lambda x: (-x[1], x[0])):
    if n >= 2:
        out.append("  %s %d" % (ip, n))
print("\n".join(out))
