import sys
from collections import defaultdict

def main():
    words = {w.lower() for w in sys.stdin.read().split()}
    groups = defaultdict(list)
    for w in words:
        groups[''.join(sorted(w))].append(w)
    result = []
    for members in groups.values():
        if len(members) < 2:
            continue
        result.append(sorted(members))
    result.sort(key=lambda g: (-len(g), g[0]))
    out = [' '.join(g) for g in result]
    if out:
        sys.stdout.write('\n'.join(out) + '\n')

main()
