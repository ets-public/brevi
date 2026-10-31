import re
import sys

MISSING = object()
INT_RE = re.compile(r'^-?\d+$')

data = {}
txs = []  # stack of undo logs: {key: previous value or MISSING}


def record(key):
    if txs and key not in txs[-1]:
        txs[-1][key] = data.get(key, MISSING)


def main():
    out = []
    for raw in sys.stdin.read().splitlines():
        parts = raw.split()
        if not parts:
            continue
        cmd, args = parts[0], parts[1:]
        n = len(args)

        if cmd == "SET" and n == 2:
            record(args[0])
            data[args[0]] = args[1]

        elif cmd == "GET" and n == 1:
            out.append(data.get(args[0], "NULL"))

        elif cmd == "DEL" and n == 1:
            k = args[0]
            if k in data:
                record(k)
                del data[k]
            else:
                out.append("NOT FOUND")

        elif cmd == "INCR" and n == 2:
            k, amt = args
            if not INT_RE.match(amt):
                out.append("ERR not an integer")
                continue
            cur = data.get(k, "0")
            if not INT_RE.match(cur):
                out.append("ERR not an integer")
                continue
            record(k)
            data[k] = str(int(cur) + int(amt))
            out.append(data[k])

        elif cmd == "COUNT" and n == 1:
            out.append(str(sum(1 for v in data.values() if v == args[0])))

        elif cmd == "KEYS" and n <= 1:
            prefix = args[0] if n == 1 else ""
            keys = sorted(k for k in data if k.startswith(prefix))
            out.append(" ".join(keys) if keys else "(empty)")

        elif cmd == "BEGIN" and n == 0:
            txs.append({})

        elif cmd == "ROLLBACK" and n == 0:
            if not txs:
                out.append("NO TRANSACTION")
            else:
                log = txs.pop()
                for k, old in log.items():
                    if old is MISSING:
                        data.pop(k, None)
                    else:
                        data[k] = old

        elif cmd == "COMMIT" and n == 0:
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
