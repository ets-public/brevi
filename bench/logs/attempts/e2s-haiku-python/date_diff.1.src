import sys
from datetime import date


def main():
    out = []
    for line in sys.stdin:
        parts = line.split()
        if len(parts) < 2:
            continue
        a = date.fromisoformat(parts[0])
        b = date.fromisoformat(parts[1])
        out.append(str(abs((b - a).days)))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
