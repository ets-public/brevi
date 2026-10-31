import sys, re
RE = re.compile(r'^(\S+) - - \[(\d{1,2}/[A-Za-z]{3}/\d{4}):(\d\d):\d\d:\d\d [+-]\d{4}\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')
req = mal = byt = bots = 0
ips = set(); cls = {}; paths = {}; hours = {}; errs = {}
for line in sys.stdin.read().split("\n"):
    line = line.rstrip("\r")
    if not line.strip(): continue
    m = RE.match(line)
    if not m:
        mal += 1; continue
    ip, _, hh, meth, path, proto, st, size, ref, ua = m.groups()
    req += 1
    byt += 0 if size == "-" else int(size)
    ips.add(ip)
    k = st[0] + "xx"; cls[k] = cls.get(k, 0) + 1
    path = path.split("?", 1)[0]
    paths[path] = paths.get(path, 0) + 1
    hours[hh] = hours.get(hh, 0) + 1
    if "bot" in ua.lower(): bots += 1
    if int(st) >= 400: errs[ip] = errs.get(ip, 0) + 1
print("requests %d" % req); print("malformed %d" % mal); print("bytes %d" % byt); print("unique_ips %d" % len(ips))
print("status")
for k in ("2xx", "3xx", "4xx", "5xx"):
    if k in cls: print("  %s %d" % (k, cls[k]))
print("top_paths")
for p, c in sorted(paths.items(), key=lambda x: (-x[1], x[0]))[:3]:
    print("  %d %s" % (c, p))
print("hours")
if hours:
    mx = max(hours.values())
    for h in sorted(hours):
        n = hours[h]
        bar = max(1, (2 * n * 20 + mx) // (2 * mx))
        print("  %s %d %s" % (h, n, "#" * bar))
print("bots %d" % bots)
print("errors_by_ip")
for ip, c in sorted(errs.items(), key=lambda x: (-x[1], x[0])):
    if c >= 2: print("  %s %d" % (ip, c))
