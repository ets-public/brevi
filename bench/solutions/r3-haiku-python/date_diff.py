import sys
from datetime import date


def main():
    out = []
    for line in sys.stdin.read().split("\n"):
        parts = line.split()
        if len(parts) < 2:
            continue
        a = date.fromisoformat(parts[0])
        b = date.fromisoformat(parts[1])
        out.append(str(abs((a - b).days)))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
