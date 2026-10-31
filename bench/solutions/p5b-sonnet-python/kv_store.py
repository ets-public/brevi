import sys
store = {}
stack = []
def setk(k, v):
    if stack and k not in stack[-1]:
        stack[-1][k] = store.get(k)
    if v is None:
        store.pop(k, None)
    else:
        store[k] = v
def toint(s):
    try:
        return int(s)
    except ValueError:
        return None
ARGS = {"SET": 2, "GET": 1, "DEL": 1, "INCR": 2, "COUNT": 1, "KEYS": 1, "BEGIN": 0, "ROLLBACK": 0, "COMMIT": 0}
for line in sys.stdin.read().split("\n"):
    p = line.split()
    if not p:
        continue
    c = p[0]
    if c not in ARGS or len(p) - 1 != ARGS[c]:
        print("ERR unknown command"); continue
    if c == "SET":
        setk(p[1], p[2])
    elif c == "GET":
        print(store.get(p[1], "NULL"))
    elif c == "DEL":
        if p[1] in store:
            setk(p[1], None)
        else:
            print("NOT FOUND")
    elif c == "INCR":
        n = toint(p[2]); cur = toint(store[p[1]]) if p[1] in store else 0
        if n is None or cur is None:
            print("ERR not an integer")
        else:
            setk(p[1], str(cur + n)); print(cur + n)
    elif c == "COUNT":
        print(sum(1 for v in store.values() if v == p[1]))
    elif c == "KEYS":
        ks = sorted(k for k in store if k.startswith(p[1]))
        print(" ".join(ks) if ks else "(empty)")
    elif c == "BEGIN":
        stack.append({})
    elif c == "ROLLBACK":
        if not stack:
            print("NO TRANSACTION")
        else:
            for k, v in stack.pop().items():
                if v is None:
                    store.pop(k, None)
                else:
                    store[k] = v
    elif c == "COMMIT":
        if not stack:
            print("NO TRANSACTION")
        else:
            stack.clear()
