import re
import sys
from collections import Counter

LINE_RE = re.compile(
    r'^(\S+) - - \[(\d{2})/[A-Za-z]{3}/\d{4}:(\d{2}):\d{2}:\d{2} [+-]\d{4}\] '
    r'"([^ "]+) ([^ "]+) ([^ "]+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$'
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
        line = raw.rstrip("\r\n")
        m = LINE_RE.match(line)
        if not m:
            malformed += 1
            continue
        ip, hour, method, path, proto, status, size, referrer, agent = (
            m.group(1), m.group(3), m.group(4), m.group(5), m.group(6),
            m.group(7), m.group(8), m.group(9), m.group(10),
        )
        requests += 1
        total_bytes += 0 if size == "-" else int(size)
        ips.add(ip)
        cls = status[0]
        status_classes[cls] += 1
        clean_path = path.split("?", 1)[0]
        paths[clean_path] += 1
        hours[hour] += 1
        if "bot" in agent.lower():
            bots += 1
        if int(status) >= 400:
            errors_by_ip[ip] += 1

    out = []
    out.append("requests %d" % requests)
    out.append("malformed %d" % malformed)
    out.append("bytes %d" % total_bytes)
    out.append("unique_ips %d" % len(ips))

    out.append("status")
    for cls in ("2", "3", "4", "5"):
        if status_classes[cls]:
            out.append("  %sxx %d" % (cls, status_classes[cls]))

    out.append("top_paths")
    ranked = sorted(paths.items(), key=lambda kv: (-kv[1], kv[0]))
    for p, c in ranked[:3]:
        out.append("  %d %s" % (c, p))

    out.append("hours")
    if hours:
        peak = max(hours.values())
        for h in sorted(hours):
            n = hours[h]
            # round half up of n * 20 / peak, at least 1
            bar_len = max(1, (n * 40 + peak) // (2 * peak))
            out.append("  %s %d %s" % (h, n, "#" * bar_len))

    out.append("bots %d" % bots)

    out.append("errors_by_ip")
    errs = [(ip, c) for ip, c in errors_by_ip.items() if c >= 2]
    errs.sort(key=lambda kv: (-kv[1], kv[0]))
    for ip, c in errs:
        out.append("  %s %d" % (ip, c))

    sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
