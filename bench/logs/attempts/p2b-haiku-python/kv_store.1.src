import re
import sys

MISSING = object()
INT_RE = re.compile(r"-?[0-9]+")


def main():
    store = {}
    txns = []  # each open transaction: dict key -> value before its first change
    out = []

    def record(key):
        if txns and key not in txns[-1]:
            txns[-1][key] = store.get(key, MISSING)

    for raw in sys.stdin.read().splitlines():
        line = raw.strip()
        if not line:
            continue
        parts = line.split(" ")
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
            key, delta = args
            cur = store.get(key, "0")
            if not INT_RE.fullmatch(cur) or not INT_RE.fullmatch(delta):
                out.append("ERR not an integer")
            else:
                new = str(int(cur) + int(delta))
                record(key)
                store[key] = new
                out.append(new)

        elif cmd == "COUNT" and len(args) == 1:
            out.append(str(sum(1 for v in store.values() if v == args[0])))

        elif cmd == "KEYS" and len(args) == 1:
            keys = sorted(k for k in store if k.startswith(args[0]))
            out.append(" ".join(keys) if keys else "(empty)")

        elif cmd == "BEGIN" and not args:
            txns.append({})

        elif cmd == "ROLLBACK" and not args:
            if not txns:
                out.append("NO TRANSACTION")
            else:
                log = txns.pop()
                for key, old in log.items():
                    if old is MISSING:
                        store.pop(key, None)
                    else:
                        store[key] = old

        elif cmd == "COMMIT" and not args:
            if not txns:
                out.append("NO TRANSACTION")
            else:
                txns.clear()

        else:
            out.append("ERR unknown command")

    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
