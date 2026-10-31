import re
import sys
from collections import Counter

LINE_RE = re.compile(
    r'^(\S+) - - \[([^\]]+)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$'
)


def main():
    data = sys.stdin.read()
    lines = data.splitlines()
    requests = 0
    malformed = 0
    total_bytes = 0
    ips = set()
    status_counts = Counter()
    path_counts = Counter()
    hour_counts = Counter()
    bots = 0
    err_counts = Counter()

    for line in lines:
        m = LINE_RE.match(line)
        if not m:
            malformed += 1
            continue
        ip, ts, _method, path, _proto, status, size, _ref, ua = m.groups()
        requests += 1
        ips.add(ip)
        total_bytes += 0 if size == "-" else int(size)
        status_counts[status[0]] += 1
        path = path.split("?", 1)[0]
        path_counts[path] += 1
        hour = ts.split(":")[1]
        hour_counts[hour] += 1
        if "bot" in ua.lower():
            bots += 1
        if int(status) >= 400:
            err_counts[ip] += 1

    out = []
    out.append(f"requests {requests}")
    out.append(f"malformed {malformed}")
    out.append(f"bytes {total_bytes}")
    out.append(f"unique_ips {len(ips)}")

    out.append("status")
    for cls in "2345":
        if status_counts[cls]:
            out.append(f"  {cls}xx {status_counts[cls]}")

    out.append("top_paths")
    ranked = sorted(path_counts.items(), key=lambda kv: (-kv[1], kv[0]))
    for path, n in ranked[:3]:
        out.append(f"  {n} {path}")

    out.append("hours")
    if hour_counts:
        mx = max(hour_counts.values())
        for hour in sorted(hour_counts):
            n = hour_counts[hour]
            width = (n * 20 * 2 + mx) // (2 * mx)
            width = max(1, width)
            out.append(f"  {hour} {n} {'#' * width}")

    out.append(f"bots {bots}")

    out.append("errors_by_ip")
    errs = [(ip, n) for ip, n in err_counts.items() if n >= 2]
    errs.sort(key=lambda kv: (-kv[1], kv[0]))
    for ip, n in errs:
        out.append(f"  {ip} {n}")

    sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
