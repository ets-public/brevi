import sys
from collections import OrderedDict


def main():
    lines = [l.strip() for l in sys.stdin.read().splitlines() if l.strip()]
    if not lines:
        return
    cap = int(lines[0])
    cache = OrderedDict()
    out = []
    for line in lines[1:]:
        parts = line.split()
        if parts[0] == "put" and len(parts) >= 3:
            key, val = parts[1], parts[2]
            if key in cache:
                cache[key] = val
                cache.move_to_end(key)
            else:
                if len(cache) >= cap:
                    cache.popitem(last=False)
                cache[key] = val
        elif parts[0] == "get" and len(parts) >= 2:
            key = parts[1]
            if key in cache:
                cache.move_to_end(key)
                out.append(cache[key])
            else:
                out.append("-1")
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
