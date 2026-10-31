import sys
import re
from collections import Counter

LINE = re.compile(r'^(\S+) - - \[([^\]]*)\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')


def main():
    text = sys.stdin.read()
    malformed = 0
    recs = []
    for line in text.splitlines():
        m = LINE.match(line)
        if not m:
            malformed += 1
            continue
        ip, ts, method, path, proto, status, size, ref, ua = m.groups()
        recs.append((ip, ts, path.split('?', 1)[0], int(status),
                     0 if size == '-' else int(size), ua))

    out = []
    out.append(f"requests {len(recs)}")
    out.append(f"malformed {malformed}")
    out.append(f"bytes {sum(r[4] for r in recs)}")
    out.append(f"unique_ips {len(set(r[0] for r in recs))}")

    out.append("status")
    cls = Counter(r[3] // 100 for r in recs)
    for c in (2, 3, 4, 5):
        if cls[c]:
            out.append(f"  {c}xx {cls[c]}")

    out.append("top_paths")
    pc = Counter(r[2] for r in recs)
    for p, c in sorted(pc.items(), key=lambda x: (-x[1], x[0]))[:3]:
        out.append(f"  {c} {p}")

    out.append("hours")
    hc = Counter(r[1].split(':')[1] for r in recs)
    if hc:
        mx = max(hc.values())
        for h in sorted(hc):
            n = hc[h]
            bar = max(1, (n * 20 * 2 + mx) // (2 * mx))
            out.append(f"  {h} {n} {'#' * bar}")

    bots = sum(1 for r in recs if 'bot' in r[5].lower())
    out.append(f"bots {bots}")

    out.append("errors_by_ip")
    ec = Counter(r[0] for r in recs if r[3] >= 400)
    for ip, n in sorted(((i, c) for i, c in ec.items() if c >= 2),
                        key=lambda x: (-x[1], x[0])):
        out.append(f"  {ip} {n}")

    print('\n'.join(out))


if __name__ == '__main__':
    main()
