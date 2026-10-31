import re
import sys
from collections import Counter

LINE = re.compile(
    r'(\S+) - - \[\d{2}/[A-Za-z]{3}/\d{4}:(\d{2}):\d{2}:\d{2} [+-]\d{4}\] '
    r'"(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "[^"]*" "([^"]*)"'
)


def main():
    lines = sys.stdin.read().split('\n')
    if lines and lines[-1] == '':
        lines.pop()
    requests = 0
    malformed = 0
    total = 0
    ips = set()
    classes = Counter()
    paths = Counter()
    hours = Counter()
    bots = 0
    errors = Counter()
    for raw in lines:
        m = LINE.fullmatch(raw.rstrip('\r'))
        if not m:
            malformed += 1
            continue
        ip, hour, _method, path, _proto, status, size, agent = m.groups()
        requests += 1
        total += 0 if size == '-' else int(size)
        ips.add(ip)
        classes[status[0]] += 1
        paths[path.split('?', 1)[0]] += 1
        hours[hour] += 1
        if 'bot' in agent.lower():
            bots += 1
        if int(status) >= 400:
            errors[ip] += 1

    out = [
        f'requests {requests}',
        f'malformed {malformed}',
        f'bytes {total}',
        f'unique_ips {len(ips)}',
        'status',
    ]
    for c in '2345':
        if classes[c]:
            out.append(f'  {c}xx {classes[c]}')
    out.append('top_paths')
    for p, n in sorted(paths.items(), key=lambda kv: (-kv[1], kv[0]))[:3]:
        out.append(f'  {n} {p}')
    out.append('hours')
    if hours:
        busiest = max(hours.values())
        for h in sorted(hours):
            n = hours[h]
            bar = max(1, (2 * n * 20 + busiest) // (2 * busiest))
            out.append(f'  {h} {n} {"#" * bar}')
    out.append(f'bots {bots}')
    out.append('errors_by_ip')
    for ip, n in sorted(errors.items(), key=lambda kv: (-kv[1], kv[0])):
        if n >= 2:
            out.append(f'  {ip} {n}')
    sys.stdout.write('\n'.join(out) + '\n')


main()
