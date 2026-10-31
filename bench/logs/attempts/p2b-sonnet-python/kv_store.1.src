import sys
store = {}
stack = []
MISSING = object()

def rec(k):
    if stack and k not in stack[-1]:
        stack[-1][k] = store.get(k, MISSING)

def isint(s):
    try:
        int(s)
        return True
    except ValueError:
        return False

NARGS = {"SET": 2, "GET": 1, "DEL": 1, "INCR": 2, "COUNT": 1, "KEYS": 1, "BEGIN": 0, "ROLLBACK": 0, "COMMIT": 0}
for line in sys.stdin.read().split('\n'):
    p = line.split()
    if not p:
        continue
    c = p[0]
    a = p[1:]
    if c not in NARGS or len(a) != NARGS[c]:
        print("ERR unknown command")
        continue
    if c == "SET":
        rec(a[0]); store[a[0]] = a[1]
    elif c == "GET":
        print(store.get(a[0], "NULL"))
    elif c == "DEL":
        if a[0] in store:
            rec(a[0]); del store[a[0]]
        else:
            print("NOT FOUND")
    elif c == "INCR":
        cur = store.get(a[0], "0")
        if not isint(cur) or not isint(a[1]):
            print("ERR not an integer")
        else:
            v = int(cur) + int(a[1])
            rec(a[0]); store[a[0]] = str(v)
            print(v)
    elif c == "COUNT":
        print(sum(1 for v in store.values() if v == a[0]))
    elif c == "KEYS":
        ks = sorted(k for k in store if k.startswith(a[0]))
        print(" ".join(ks) if ks else "(empty)")
    elif c == "BEGIN":
        stack.append({})
    elif c == "ROLLBACK":
        if not stack:
            print("NO TRANSACTION")
        else:
            for k, v in stack.pop().items():
                if v is MISSING:
                    store.pop(k, None)
                else:
                    store[k] = v
    elif c == "COMMIT":
        if not stack:
            print("NO TRANSACTION")
        else:
            stack.clear()
