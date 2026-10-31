import sys, re
from collections import Counter
pat = re.compile(r'^(\S+) - - \[([^\]]+)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')
req = mal = total = bots = 0
ips = set()
status = Counter(); paths = Counter(); hours = Counter(); errs = Counter()
for line in sys.stdin.read().split('\n'):
    line = line.rstrip('\r')
    if not line.strip():
        continue
    m = pat.match(line)
    if not m:
        mal += 1
        continue
    ip, ts, meth, path, proto, st, size, ref, ua = m.groups()
    req += 1
    total += 0 if size == '-' else int(size)
    ips.add(ip)
    status[st[0]] += 1
    paths[path.split('?')[0]] += 1
    mm = re.match(r'^[^:]*:(\d{2}):', ts)
    hours[mm.group(1) if mm else '00'] += 1
    if 'bot' in ua.lower():
        bots += 1
    if int(st) >= 400:
        errs[ip] += 1
out = ["requests %d" % req, "malformed %d" % mal, "bytes %d" % total, "unique_ips %d" % len(ips), "status"]
for c in '2345':
    if status[c]:
        out.append("  %sxx %d" % (c, status[c]))
out.append("top_paths")
for p, n in sorted(paths.items(), key=lambda x: (-x[1], x[0]))[:3]:
    out.append("  %d %s" % (n, p))
out.append("hours")
if hours:
    mx = max(hours.values())
    for h in sorted(hours):
        n = hours[h]
        k = max(1, (n * 40 + mx) // (2 * mx))
        out.append("  %s %d %s" % (h, n, '#' * k))
out.append("bots %d" % bots)
out.append("errors_by_ip")
for ip, n in sorted(errs.items(), key=lambda x: (-x[1], x[0])):
    if n >= 2:
        out.append("  %s %d" % (ip, n))
print("\n".join(out))
