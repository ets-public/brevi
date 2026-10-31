import sys, re
from collections import Counter
pat = re.compile(r'(\S+) - - \[(\d+/\w+/\d+:(\d\d):\d\d:\d\d [^\]]*)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"')
req = mal = total = bots = 0
ips = set(); st = Counter(); paths = Counter(); hours = Counter(); errs = Counter()
for line in sys.stdin.read().split('\n'):
    if not line.strip():
        continue
    m = pat.fullmatch(line.strip())
    if not m:
        mal += 1
        continue
    ip, _, hh, meth, path, proto, status, size, ref, ua = m.groups()
    req += 1
    total += 0 if size == '-' else int(size)
    ips.add(ip)
    st[status[0] + 'xx'] += 1
    paths[path.split('?')[0]] += 1
    hours[hh] += 1
    if 'bot' in ua.lower():
        bots += 1
    if int(status) >= 400:
        errs[ip] += 1
print('requests', req)
print('malformed', mal)
print('bytes', total)
print('unique_ips', len(ips))
print('status')
for c in ('2xx', '3xx', '4xx', '5xx'):
    if c in st:
        print('  %s %d' % (c, st[c]))
print('top_paths')
for p, n in sorted(paths.items(), key=lambda x: (-x[1], x[0]))[:3]:
    print('  %d %s' % (n, p))
print('hours')
if hours:
    mx = max(hours.values())
    for h in sorted(hours):
        n = hours[h]
        b = max(1, (n * 40 + mx) // (2 * mx))
        print('  %s %d %s' % (h, n, '#' * b))
print('bots', bots)
print('errors_by_ip')
for ip, n in sorted(errs.items(), key=lambda x: (-x[1], x[0])):
    if n >= 2:
        print('  %s %d' % (ip, n))
