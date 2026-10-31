import re
import sys
from collections import Counter

LINE = re.compile(r'^(\S+) - - \[([^\]]+)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')

requests = 0
malformed = 0
total_bytes = 0
ips = set()
classes = Counter()
paths = Counter()
hours = Counter()
bots = 0
err_by_ip = Counter()

for raw in sys.stdin:
    line = raw.rstrip("\r\n")
    m = LINE.match(line)
    if not m:
        malformed += 1
        continue
    ip, ts, method, path, proto, status, size, ref, ua = m.groups()
    requests += 1
    st = int(status)
    total_bytes += 0 if size == "-" else int(size)
    ips.add(ip)
    classes[status[0] + "xx"] += 1
    path = path.split("?", 1)[0]
    paths[path] += 1
    hour = ts.split(":")[1]
    hours[hour] += 1
    if "bot" in ua.lower():
        bots += 1
    if st >= 400:
        err_by_ip[ip] += 1

out = []
out.append(f"requests {requests}")
out.append(f"malformed {malformed}")
out.append(f"bytes {total_bytes}")
out.append(f"unique_ips {len(ips)}")
out.append("status")
for c in ["2xx", "3xx", "4xx", "5xx"]:
    if classes[c]:
        out.append(f"  {c} {classes[c]}")
out.append("top_paths")
for p, n in sorted(paths.items(), key=lambda kv: (-kv[1], kv[0]))[:3]:
    out.append(f"  {n} {p}")
out.append("hours")
if hours:
    mx = max(hours.values())
    for h in sorted(hours):
        n = hours[h]
        bar_len = max(1, (n * 20 * 2 + mx) // (2 * mx))
        out.append(f"  {h} {n} {'#' * bar_len}")
out.append(f"bots {bots}")
out.append("errors_by_ip")
for ip, n in sorted(err_by_ip.items(), key=lambda kv: (-kv[1], kv[0])):
    if n >= 2:
        out.append(f"  {ip} {n}")

print("\n".join(out))
