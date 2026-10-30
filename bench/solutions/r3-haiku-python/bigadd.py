import sys


def main():
    out = []
    for line in sys.stdin.read().split("\n"):
        parts = line.split()
        if len(parts) < 2:
            continue
        a = int(parts[0])
        b = int(parts[1])
        out.append(str(a + b))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
