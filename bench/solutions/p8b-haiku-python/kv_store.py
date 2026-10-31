import re
import sys

store = {}
snapshots = []
INT = re.compile(r"[-+]?[0-9]+")

for raw in sys.stdin:
    parts = raw.split()
    if not parts:
        continue
    cmd, args = parts[0], parts[1:]
    if cmd == "SET" and len(args) == 2:
        store[args[0]] = args[1]
    elif cmd == "GET" and len(args) == 1:
        print(store.get(args[0], "NULL"))
    elif cmd == "DEL" and len(args) == 1:
        if args[0] in store:
            del store[args[0]]
        else:
            print("NOT FOUND")
    elif cmd == "INCR" and len(args) == 2:
        k, n = args
        cur = store.get(k, "0")
        if not INT.fullmatch(cur) or not INT.fullmatch(n):
            print("ERR not an integer")
        else:
            val = int(cur) + int(n)
            store[k] = str(val)
            print(val)
    elif cmd == "COUNT" and len(args) == 1:
        print(sum(1 for v in store.values() if v == args[0]))
    elif cmd == "KEYS" and len(args) == 1:
        ks = sorted(k for k in store if k.startswith(args[0]))
        print(" ".join(ks) if ks else "(empty)")
    elif cmd == "BEGIN" and not args:
        snapshots.append(dict(store))
    elif cmd == "ROLLBACK" and not args:
        if snapshots:
            store = snapshots.pop()
        else:
            print("NO TRANSACTION")
    elif cmd == "COMMIT" and not args:
        if snapshots:
            snapshots.clear()
        else:
            print("NO TRANSACTION")
    else:
        print("ERR unknown command")
