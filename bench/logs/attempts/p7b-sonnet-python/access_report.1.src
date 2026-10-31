import sys, re
from collections import Counter
rx = re.compile(r'^(\S+) - - \[([^\]]+)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')
req = mal = tot = bots = 0
ips = set()
sc = Counter(); paths = Counter(); hrs = Counter(); err = Counter()
for line in sys.stdin.read().split('\n'):
    line = line.rstrip('\r')
    if not line.strip():
        continue
    m = rx.match(line)
    if not m:
        mal += 1
        continue
    ip, ts, meth, path, proto, st, size, ref, ua = m.groups()
    mm = re.match(r'^\d+/\w+/\d+:(\d\d):\d\d:\d\d ', ts)
    if not mm:
        mal += 1
        continue
    req += 1
    tot += 0 if size == '-' else int(size)
    ips.add(ip)
    sc[st[0] + 'xx'] += 1
    paths[path.split('?')[0]] += 1
    hrs[mm.group(1)] += 1
    if 'bot' in ua.lower():
        bots += 1
    if int(st) >= 400:
        err[ip] += 1
print('requests', req)
print('malformed', mal)
print('bytes', tot)
print('unique_ips', len(ips))
print('status')
for c in ('2xx', '3xx', '4xx', '5xx'):
    if c in sc:
        print('  %s %d' % (c, sc[c]))
print('top_paths')
for p, n in sorted(paths.items(), key=lambda x: (-x[1], x[0]))[:3]:
    print('  %d %s' % (n, p))
print('hours')
if hrs:
    mx = max(hrs.values())
    for h in sorted(hrs):
        n = hrs[h]
        b = max(1, (n * 20 * 2 + mx) // (2 * mx))
        print('  %s %d %s' % (h, n, '#' * b))
print('bots', bots)
print('errors_by_ip')
for ip, n in sorted(err.items(), key=lambda x: (-x[1], x[0])):
    if n >= 2:
        print('  %s %d' % (ip, n))
