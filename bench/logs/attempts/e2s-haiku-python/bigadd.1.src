import sys

def main():
    out = []
    for line in sys.stdin:
        parts = line.split()
        if len(parts) < 2:
            continue
        a, b = int(parts[0]), int(parts[1])
        out.append(str(a + b))
    if out:
        sys.stdout.write("\n".join(out) + "\n")

main()
