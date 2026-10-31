import sys, re

store = {}
stack = []
ARGS = {"SET": 2, "GET": 1, "DEL": 1, "INCR": 2, "COUNT": 1, "KEYS": 1,
        "BEGIN": 0, "ROLLBACK": 0, "COMMIT": 0}
isint = lambda s: re.fullmatch(r'[+-]?\d+', s) is not None
for line in sys.stdin.read().split("\n"):
    p = line.split()
    if not p:
        continue
    c = p[0]
    a = p[1:]
    if c not in ARGS or len(a) != ARGS[c]:
        print("ERR unknown command")
        continue
    if c == "SET":
        store[a[0]] = a[1]
    elif c == "GET":
        print(store.get(a[0], "NULL"))
    elif c == "DEL":
        if a[0] in store:
            del store[a[0]]
        else:
            print("NOT FOUND")
    elif c == "INCR":
        cur = store.get(a[0], "0")
        if not isint(cur) or not isint(a[1]):
            print("ERR not an integer")
        else:
            v = str(int(cur) + int(a[1]))
            store[a[0]] = v
            print(v)
    elif c == "COUNT":
        print(sum(1 for v in store.values() if v == a[0]))
    elif c == "KEYS":
        ks = sorted(k for k in store if k.startswith(a[0]))
        print(" ".join(ks) if ks else "(empty)")
    elif c == "BEGIN":
        stack.append(dict(store))
    elif c == "ROLLBACK":
        if not stack:
            print("NO TRANSACTION")
        else:
            store = stack.pop()
    elif c == "COMMIT":
        if not stack:
            print("NO TRANSACTION")
        else:
            stack = []
