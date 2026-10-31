import re
import sys
from collections import Counter, defaultdict

LINE_RE = re.compile(
    r'^(\S+) - - \[(\d{2}/[A-Za-z]{3}/\d{4}:(\d{2}):\d{2}:\d{2} [+-]\d{4})\] '
    r'"([^"]*)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$'
)


def main():
    requests = 0
    malformed = 0
    total_bytes = 0
    ips = set()
    status_classes = Counter()
    paths = Counter()
    hours = Counter()
    bots = 0
    errors_by_ip = Counter()

    for raw in sys.stdin:
        line = raw.rstrip('\r\n')
        m = LINE_RE.match(line)
        if not m:
            malformed += 1
            continue
        ip = m.group(1)
        hour = m.group(3)
        req = m.group(4).split(' ')
        if len(req) != 3:
            malformed += 1
            continue
        method, target, proto = req
        status = int(m.group(5))
        size_s = m.group(6)
        ua = m.group(8)

        requests += 1
        total_bytes += 0 if size_s == '-' else int(size_s)
        ips.add(ip)
        status_classes[status // 100] += 1
        path = target.split('?', 1)[0]
        paths[path] += 1
        hours[hour] += 1
        if 'bot' in ua.lower():
            bots += 1
        if status >= 400:
            errors_by_ip[ip] += 1

    out = []
    out.append('requests %d' % requests)
    out.append('malformed %d' % malformed)
    out.append('bytes %d' % total_bytes)
    out.append('unique_ips %d' % len(ips))

    out.append('status')
    for c in (2, 3, 4, 5):
        if status_classes[c]:
            out.append('  %dxx %d' % (c, status_classes[c]))

    out.append('top_paths')
    top = sorted(paths.items(), key=lambda kv: (-kv[1], kv[0]))[:3]
    for p, n in top:
        out.append('  %d %s' % (n, p))

    out.append('hours')
    if hours:
        mx = max(hours.values())
        for h in sorted(hours):
            n = hours[h]
            # round(n * 20 / mx) with round half up, at least 1
            bar_len = max(1, (n * 40 + mx) // (2 * mx))
            out.append('  %s %d %s' % (h, n, '#' * bar_len))

    out.append('bots %d' % bots)

    out.append('errors_by_ip')
    errs = [(ip, n) for ip, n in errors_by_ip.items() if n >= 2]
    errs.sort(key=lambda kv: (-kv[1], kv[0]))
    for ip, n in errs:
        out.append('  %s %d' % (ip, n))

    sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
