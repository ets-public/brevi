import sys


def main():
    ivs = []
    for line in sys.stdin:
        parts = line.split()
        if len(parts) < 2:
            continue
        ivs.append((int(parts[0]), int(parts[1])))
    ivs.sort()
    merged = []
    for s, e in ivs:
        if merged and s <= merged[-1][1]:
            if e > merged[-1][1]:
                merged[-1][1] = e
        else:
            merged.append([s, e])
    out = [f"{s} {e}" for s, e in merged]
    total = sum(e - s for s, e in merged)
    out.append(f"total {total}")
    sys.stdout.write("\n".join(out) + "\n")


main()
