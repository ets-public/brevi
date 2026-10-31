import sys
import re
from collections import Counter

LINE = re.compile(
    r'^(\S+) - - \[(\d{2}/\w{3}/\d{4}:\d{2}:\d{2}:\d{2} [+-]\d{4})\] '
    r'"(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "[^"]*" "([^"]*)"$'
)


def main():
    requests = 0
    malformed = 0
    total = 0
    ips = set()
    classes = Counter()
    paths = Counter()
    hours = Counter()
    bots = 0
    err_by_ip = Counter()

    for line in sys.stdin.read().splitlines():
        m = LINE.match(line)
        if not m:
            malformed += 1
            continue
        ip, ts, method, path, proto, status, size, ua = m.groups()
        path = path.split('?', 1)[0]
        requests += 1
        total += 0 if size == '-' else int(size)
        ips.add(ip)
        classes[status[0] + 'xx'] += 1
        paths[path] += 1
        hours[ts.split(':')[1]] += 1
        if 'bot' in ua.lower():
            bots += 1
        if int(status) >= 400:
            err_by_ip[ip] += 1

    out = [
        f"requests {requests}",
        f"malformed {malformed}",
        f"bytes {total}",
        f"unique_ips {len(ips)}",
        "status",
    ]
    for c in ['2xx', '3xx', '4xx', '5xx']:
        if classes[c]:
            out.append(f"  {c} {classes[c]}")
    out.append("top_paths")
    for p, c in sorted(paths.items(), key=lambda kv: (-kv[1], kv[0]))[:3]:
        out.append(f"  {c} {p}")
    out.append("hours")
    if hours:
        mx = max(hours.values())
        for h in sorted(hours):
            n = hours[h]
            bar = '#' * max(1, (2 * n * 20 + mx) // (2 * mx))
            out.append(f"  {h} {n} {bar}")
    out.append(f"bots {bots}")
    out.append("errors_by_ip")
    for ip, n in sorted(err_by_ip.items(), key=lambda kv: (-kv[1], kv[0])):
        if n >= 2:
            out.append(f"  {ip} {n}")
    print("\n".join(out))


main()
