import sys, re
from collections import Counter

RE = re.compile(r'^(\S+) - - \[(\d{1,2}/\w{3}/\d{4}):(\d{2}):\d{2}:\d{2} [+-]\d{4}\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')
req = mal = tot = bots = 0
ips = set()
status = Counter(); paths = Counter(); hours = Counter(); errs = Counter()
for line in sys.stdin:
    line = line.rstrip('\r\n')
    if not line.strip():
        continue
    m = RE.match(line.strip())
    if not m:
        mal += 1
        continue
    ip, _, hh, meth, path, proto, st, size, ref, ua = m.groups()
    req += 1
    tot += 0 if size == '-' else int(size)
    ips.add(ip)
    st = int(st)
    status[st // 100] += 1
    paths[path.split('?', 1)[0]] += 1
    hours[int(hh)] += 1
    if 'bot' in ua.lower():
        bots += 1
    if st >= 400:
        errs[ip] += 1
print('requests', req)
print('malformed', mal)
print('bytes', tot)
print('unique_ips', len(ips))
print('status')
for k in (2, 3, 4, 5):
    if status[k]:
        print('  %dxx %d' % (k, status[k]))
print('top_paths')
for p, c in sorted(paths.items(), key=lambda x: (-x[1], x[0]))[:3]:
    print('  %d %s' % (c, p))
print('hours')
if hours:
    mx = max(hours.values())
    for h in sorted(hours):
        n = hours[h]
        bar = max(1, int((n * 20 / mx) + 0.5))
        print('  %02d %d %s' % (h, n, '#' * bar))
print('bots', bots)
print('errors_by_ip')
for ip, c in sorted(errs.items(), key=lambda x: (-x[1], x[0])):
    if c >= 2:
        print('  %s %d' % (ip, c))
