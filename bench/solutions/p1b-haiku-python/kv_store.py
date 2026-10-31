import re
import sys

store = {}
logs = []  # one undo log per open transaction; each entry is (key, old value or None)
_MISSING = object()


def record(key):
    if logs:
        logs[-1].append((key, store.get(key, _MISSING)))


def is_int(s):
    return re.fullmatch(r"[+-]?\d+", s) is not None


def main():
    out = []
    for raw in sys.stdin.read().splitlines():
        line = raw.strip()
        if not line:
            continue
        parts = line.split()
        cmd = parts[0]
        args = parts[1:]

        if cmd == "SET" and len(args) == 2:
            record(args[0])
            store[args[0]] = args[1]
        elif cmd == "GET" and len(args) == 1:
            out.append(store.get(args[0], "NULL"))
        elif cmd == "DEL" and len(args) == 1:
            if args[0] in store:
                record(args[0])
                del store[args[0]]
            else:
                out.append("NOT FOUND")
        elif cmd == "INCR" and len(args) == 2:
            key, n = args
            cur = store.get(key, "0")
            if not is_int(cur) or not is_int(n):
                out.append("ERR not an integer")
            else:
                new = str(int(cur) + int(n))
                record(key)
                store[key] = new
                out.append(new)
        elif cmd == "COUNT" and len(args) == 1:
            out.append(str(sum(1 for v in store.values() if v == args[0])))
        elif cmd == "KEYS" and len(args) == 1:
            keys = sorted(k for k in store if k.startswith(args[0]))
            out.append(" ".join(keys) if keys else "(empty)")
        elif cmd == "BEGIN" and not args:
            logs.append([])
        elif cmd == "ROLLBACK" and not args:
            if not logs:
                out.append("NO TRANSACTION")
            else:
                for key, old in reversed(logs.pop()):
                    if old is _MISSING:
                        store.pop(key, None)
                    else:
                        store[key] = old
        elif cmd == "COMMIT" and not args:
            if not logs:
                out.append("NO TRANSACTION")
            else:
                logs.clear()
        else:
            out.append("ERR unknown command")

    sys.stdout.write("".join(line + "\n" for line in out))


if __name__ == "__main__":
    main()
