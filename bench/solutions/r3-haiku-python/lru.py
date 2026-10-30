import sys
from collections import OrderedDict


def main():
    lines = sys.stdin.read().split("\n")
    idx = 0
    while idx < len(lines) and not lines[idx].strip():
        idx += 1
    if idx >= len(lines):
        return
    cap = int(lines[idx].strip())
    idx += 1
    cache = OrderedDict()
    out = []
    for line in lines[idx:]:
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "put" and len(parts) >= 3:
            key = parts[1]
            val = parts[2]
            if key in cache:
                cache.move_to_end(key)
                cache[key] = val
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
