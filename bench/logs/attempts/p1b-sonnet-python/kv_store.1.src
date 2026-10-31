import sys, re

store = {}
stack = []
MISSING = object()


def record(k):
    if stack and k not in stack[-1]:
        stack[-1][k] = store.get(k, MISSING)


def isint(s):
    return re.match(r'^[+-]?\d+$', s) is not None


for line in sys.stdin.read().split("\n"):
    parts = line.split()
    if not parts:
        continue
    cmd = parts[0]
    a = parts[1:]
    if cmd == "SET" and len(a) == 2:
        record(a[0])
        store[a[0]] = a[1]
    elif cmd == "GET" and len(a) == 1:
        print(store.get(a[0], "NULL"))
    elif cmd == "DEL" and len(a) == 1:
        if a[0] in store:
            record(a[0])
            del store[a[0]]
        else:
            print("NOT FOUND")
    elif cmd == "INCR" and len(a) == 2 and isint(a[1]):
        cur = store.get(a[0], "0")
        if not isint(cur):
            print("ERR not an integer")
        else:
            v = int(cur) + int(a[1])
            record(a[0])
            store[a[0]] = str(v)
            print(v)
    elif cmd == "COUNT" and len(a) == 1:
        print(sum(1 for v in store.values() if v == a[0]))
    elif cmd == "KEYS" and len(a) == 1:
        ks = sorted(k for k in store if k.startswith(a[0]))
        print(" ".join(ks) if ks else "(empty)")
    elif cmd == "BEGIN" and not a:
        stack.append({})
    elif cmd == "ROLLBACK" and not a:
        if not stack:
            print("NO TRANSACTION")
        else:
            for k, v in stack.pop().items():
                if v is MISSING:
                    store.pop(k, None)
                else:
                    store[k] = v
    elif cmd == "COMMIT" and not a:
        if not stack:
            print("NO TRANSACTION")
        else:
            stack.clear()
    else:
        print("ERR unknown command")
