import re
import sys
from collections import Counter, defaultdict

LINE_RE = re.compile(
    r'^(\S+) - - \[([^\]]+)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$'
)
HOUR_RE = re.compile(r'\d{2}/\w{3}/\d{4}:(\d{2}):')


def main():
    data = sys.stdin.read()
    lines = data.splitlines()

    requests = 0
    malformed = 0
    total_bytes = 0
    ips = set()
    status_classes = Counter()
    path_counts = Counter()
    hour_counts = Counter()
    bots = 0
    errors_per_ip = Counter()

    for line in lines:
        m = LINE_RE.match(line)
        if not m:
            malformed += 1
            continue
        ip, ts, method, path, proto, status, size, referrer, ua = m.groups()
        hm = HOUR_RE.search(ts)
        if not hm:
            malformed += 1
            continue

        requests += 1
        ips.add(ip)
        if size != '-':
            total_bytes += int(size)
        status_classes[status[0]] += 1
        path = path.split('?', 1)[0]
        path_counts[path] += 1
        hour_counts[hm.group(1)] += 1
        if 'bot' in ua.lower():
            bots += 1
        if int(status) >= 400:
            errors_per_ip[ip] += 1

    out = []
    out.append(f'requests {requests}')
    out.append(f'malformed {malformed}')
    out.append(f'bytes {total_bytes}')
    out.append(f'unique_ips {len(ips)}')

    out.append('status')
    for cls in sorted(status_classes):
        out.append(f'  {cls}xx {status_classes[cls]}')

    out.append('top_paths')
    top = sorted(path_counts.items(), key=lambda kv: (-kv[1], kv[0]))[:3]
    for path, n in top:
        out.append(f'  {n} {path}')

    out.append('hours')
    if hour_counts:
        peak = max(hour_counts.values())
        for hour in sorted(hour_counts):
            n = hour_counts[hour]
            width = max(1, (n * 20 * 2 + peak) // (2 * peak))
            out.append(f'  {hour} {n} {"#" * width}')

    out.append(f'bots {bots}')

    out.append('errors_by_ip')
    bad = [(ip, n) for ip, n in errors_per_ip.items() if n >= 2]
    bad.sort(key=lambda kv: (-kv[1], kv[0]))
    for ip, n in bad:
        out.append(f'  {ip} {n}')

    sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
