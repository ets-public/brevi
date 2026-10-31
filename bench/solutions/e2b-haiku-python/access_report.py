import re
import sys
from collections import Counter

LINE_RE = re.compile(
    r'^(\S+) - - \[(\d{2}/\w{3}/\d{4}:(\d{2}):\d{2}:\d{2} [+-]\d{4})\] '
    r'"([^ "]+) ([^ "]+) ([^ "]+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$'
)


def main():
    data = sys.stdin.read()
    lines = data.splitlines()

    requests = 0
    malformed = 0
    total_bytes = 0
    ips = set()
    classes = Counter()
    paths = Counter()
    hours = Counter()
    bots = 0
    errors = Counter()

    for line in lines:
        m = LINE_RE.match(line)
        if not m:
            malformed += 1
            continue
        ip, _ts, hour, _method, raw_path, _proto, status, size, _ref, agent = m.groups()
        requests += 1
        total_bytes += 0 if size == '-' else int(size)
        ips.add(ip)
        code = int(status)
        classes[code // 100] += 1
        path = raw_path.split('?', 1)[0]
        paths[path] += 1
        hours[hour] += 1
        if 'bot' in agent.lower():
            bots += 1
        if code >= 400:
            errors[ip] += 1

    out = []
    out.append(f"requests {requests}")
    out.append(f"malformed {malformed}")
    out.append(f"bytes {total_bytes}")
    out.append(f"unique_ips {len(ips)}")

    out.append("status")
    for c in (2, 3, 4, 5):
        if classes.get(c):
            out.append(f"  {c}xx {classes[c]}")

    out.append("top_paths")
    top = sorted(paths.items(), key=lambda kv: (-kv[1], kv[0]))[:3]
    for p, n in top:
        out.append(f"  {n} {p}")

    out.append("hours")
    if hours:
        mx = max(hours.values())
        for h in sorted(hours):
            n = hours[h]
            width = max(1, (n * 20 * 2 + mx) // (2 * mx))
            out.append(f"  {h} {n} {'#' * width}")

    out.append(f"bots {bots}")

    out.append("errors_by_ip")
    bad = [(ip, n) for ip, n in errors.items() if n >= 2]
    bad.sort(key=lambda kv: (-kv[1], kv[0]))
    for ip, n in bad:
        out.append(f"  {ip} {n}")

    sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
