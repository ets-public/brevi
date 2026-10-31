import re
import sys

INT_RE = re.compile(r"-?[0-9]+")
MISSING = object()


def main():
    data = {}
    txs = []  # stack of undo logs: list of (key, previous value or MISSING)
    out = []

    def record(key):
        if txs:
            txs[-1].append((key, data.get(key, MISSING)))

    def set_key(key, value):
        record(key)
        data[key] = value

    def del_key(key):
        record(key)
        del data[key]

    for raw in sys.stdin.read().splitlines():
        line = raw.strip()
        if line == "":
            continue
        parts = line.split()
        cmd = parts[0]
        args = parts[1:]

        if cmd == "SET" and len(args) == 2:
            set_key(args[0], args[1])
        elif cmd == "GET" and len(args) == 1:
            out.append(data.get(args[0], "NULL"))
        elif cmd == "DEL" and len(args) == 1:
            if args[0] in data:
                del_key(args[0])
            else:
                out.append("NOT FOUND")
        elif cmd == "INCR" and len(args) == 2:
            key, n = args
            if not INT_RE.fullmatch(n):
                out.append("ERR not an integer")
                continue
            cur = data.get(key, "0")
            if not INT_RE.fullmatch(cur):
                out.append("ERR not an integer")
                continue
            new = str(int(cur) + int(n))
            set_key(key, new)
            out.append(new)
        elif cmd == "COUNT" and len(args) == 1:
            out.append(str(sum(1 for v in data.values() if v == args[0])))
        elif cmd == "KEYS" and len(args) == 1:
            matches = sorted(k for k in data if k.startswith(args[0]))
            out.append(" ".join(matches) if matches else "(empty)")
        elif cmd == "BEGIN" and len(args) == 0:
            txs.append([])
        elif cmd == "ROLLBACK" and len(args) == 0:
            if not txs:
                out.append("NO TRANSACTION")
            else:
                log = txs.pop()
                for key, prev in reversed(log):
                    if prev is MISSING:
                        data.pop(key, None)
                    else:
                        data[key] = prev
        elif cmd == "COMMIT" and len(args) == 0:
            if not txs:
                out.append("NO TRANSACTION")
            else:
                txs.clear()
        else:
            out.append("ERR unknown command")

    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
