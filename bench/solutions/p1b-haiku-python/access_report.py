import re
import sys
from collections import Counter, defaultdict

LINE = re.compile(
    r'^(\S+) - - \[([^\]]+)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$'
)
HOUR = re.compile(r'\d{2}/\w{3}/\d{4}:(\d{2}):')


def main():
    requests = 0
    malformed = 0
    total_bytes = 0
    ips = set()
    classes = Counter()
    paths = Counter()
    hours = Counter()
    bots = 0
    errors = defaultdict(int)

    for line in sys.stdin.read().splitlines():
        m = LINE.match(line)
        if not m:
            malformed += 1
            continue
        ip, ts, _method, path, _proto, status, size, _ref, agent = m.groups()
        requests += 1
        ips.add(ip)
        code = int(status)
        classes[status[0]] += 1
        total_bytes += 0 if size == '-' else int(size)
        path = path.split('?', 1)[0]
        paths[path] += 1
        h = HOUR.search(ts)
        if h:
            hours[h.group(1)] += 1
        if 'bot' in agent.lower():
            bots += 1
        if code >= 400:
            errors[ip] += 1

    out = []
    out.append(f'requests {requests}')
    out.append(f'malformed {malformed}')
    out.append(f'bytes {total_bytes}')
    out.append(f'unique_ips {len(ips)}')

    out.append('status')
    for c in sorted(classes):
        out.append(f'  {c}xx {classes[c]}')

    out.append('top_paths')
    for p, n in sorted(paths.items(), key=lambda kv: (-kv[1], kv[0]))[:3]:
        out.append(f'  {n} {p}')

    out.append('hours')
    if hours:
        mx = max(hours.values())
        for hh in sorted(hours):
            n = hours[hh]
            bar_len = max(1, (n * 20 * 2 + mx) // (2 * mx))
            out.append(f'  {hh} {n} {"#" * bar_len}')

    out.append(f'bots {bots}')

    out.append('errors_by_ip')
    for ip, n in sorted(errors.items(), key=lambda kv: (-kv[1], kv[0])):
        if n >= 2:
            out.append(f'  {ip} {n}')

    sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
