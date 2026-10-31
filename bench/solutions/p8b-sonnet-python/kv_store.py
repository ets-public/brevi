import sys, re

store = {}
stack = []
intre = re.compile(r'^[+-]?\d+$')

def setk(k, v):
    if stack:
        stack[-1].append((k, store.get(k)))
    if v is None:
        store.pop(k, None)
    else:
        store[k] = v

out = []
for line in sys.stdin.read().split("\n"):
    p = line.split()
    if not p:
        continue
    c = p[0]
    n = len(p)
    if c == "SET" and n == 3:
        setk(p[1], p[2])
    elif c == "GET" and n == 2:
        out.append(store.get(p[1], "NULL"))
    elif c == "DEL" and n == 2:
        if p[1] in store:
            setk(p[1], None)
        else:
            out.append("NOT FOUND")
    elif c == "INCR" and n == 3 and intre.match(p[2]):
        cur = store.get(p[1], "0")
        if not intre.match(cur):
            out.append("ERR not an integer")
        else:
            v = str(int(cur) + int(p[2]))
            setk(p[1], v)
            out.append(v)
    elif c == "COUNT" and n == 2:
        out.append(str(sum(1 for v in store.values() if v == p[1])))
    elif c == "KEYS" and n == 2:
        ks = sorted(k for k in store if k.startswith(p[1]))
        out.append(" ".join(ks) if ks else "(empty)")
    elif c == "BEGIN" and n == 1:
        stack.append([])
    elif c == "ROLLBACK" and n == 1:
        if not stack:
            out.append("NO TRANSACTION")
        else:
            log = stack.pop()
            for k, old in reversed(log):
                if old is None:
                    store.pop(k, None)
                else:
                    store[k] = old
    elif c == "COMMIT" and n == 1:
        if not stack:
            out.append("NO TRANSACTION")
        else:
            stack = []
    else:
        out.append("ERR unknown command")
if out:
    print("\n".join(out))
