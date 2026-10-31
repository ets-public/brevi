import sys


def main():
    words = {w.lower() for w in sys.stdin.read().split()}
    groups = {}
    for w in words:
        groups.setdefault("".join(sorted(w)), []).append(w)
    result = [sorted(g) for g in groups.values() if len(g) > 1]
    result.sort(key=lambda g: (-len(g), g[0]))
    out = [" ".join(g) for g in result]
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
