import sys

_MISSING = object()


def main():
    data = {}
    # Stack of open transactions. Each maps key -> value before the first
    # change to that key inside that transaction (or _MISSING).
    txns = []

    def record(key):
        for journal in txns:
            if key not in journal:
                journal[key] = data[key] if key in data else _MISSING

    def set_key(key, value):
        record(key)
        data[key] = value

    def del_key(key):
        record(key)
        del data[key]

    out = []
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        parts = line.split()
        cmd = parts[0]
        args = parts[1:]

        if cmd == "SET" and len(args) == 2:
            set_key(args[0], args[1])
        elif cmd == "GET" and len(args) == 1:
            out.append(data[args[0]] if args[0] in data else "NULL")
        elif cmd == "DEL" and len(args) == 1:
            if args[0] in data:
                del_key(args[0])
            else:
                out.append("NOT FOUND")
        elif cmd == "INCR" and len(args) == 2:
            key = args[0]
            try:
                delta = int(args[1])
                cur = int(data.get(key, "0"))
            except ValueError:
                out.append("ERR not an integer")
                continue
            new_val = str(cur + delta)
            set_key(key, new_val)
            out.append(new_val)
        elif cmd == "COUNT" and len(args) == 1:
            out.append(str(sum(1 for v in data.values() if v == args[0])))
        elif cmd == "KEYS" and len(args) == 1:
            keys = sorted(k for k in data if k.startswith(args[0]))
            out.append(" ".join(keys) if keys else "(empty)")
        elif cmd == "BEGIN" and len(args) == 0:
            txns.append({})
        elif cmd == "ROLLBACK" and len(args) == 0:
            if not txns:
                out.append("NO TRANSACTION")
            else:
                journal = txns.pop()
                for key, old in journal.items():
                    if old is _MISSING:
                        data.pop(key, None)
                    else:
                        data[key] = old
        elif cmd == "COMMIT" and len(args) == 0:
            if not txns:
                out.append("NO TRANSACTION")
            else:
                # Keep all changes and close every open transaction.
                txns.clear()
        else:
            out.append("ERR unknown command")

    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
