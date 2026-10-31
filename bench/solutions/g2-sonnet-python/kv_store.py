import sys
def main():
    d = {}
    stack = []
    out = []
    for line in sys.stdin.read().split("\n"):
        p = line.split()
        if not p:
            continue
        c, a = p[0], p[1:]
        if c == "SET" and len(a) == 2:
            d[a[0]] = a[1]
        elif c == "GET" and len(a) == 1:
            out.append(d.get(a[0], "NULL"))
        elif c == "DEL" and len(a) == 1:
            if a[0] in d:
                del d[a[0]]
            else:
                out.append("NOT FOUND")
        elif c == "INCR" and len(a) == 2:
            try:
                n = int(a[1])
                cur = int(d.get(a[0], "0"))
            except ValueError:
                out.append("ERR not an integer")
                continue
            d[a[0]] = str(cur + n)
            out.append(d[a[0]])
        elif c == "COUNT" and len(a) == 1:
            out.append(str(sum(1 for v in d.values() if v == a[0])))
        elif c == "KEYS" and len(a) == 1:
            ks = sorted(k for k in d if k.startswith(a[0]))
            out.append(" ".join(ks) if ks else "(empty)")
        elif c == "BEGIN" and not a:
            stack.append(dict(d))
        elif c == "ROLLBACK" and not a:
            if stack:
                d = stack.pop()
            else:
                out.append("NO TRANSACTION")
        elif c == "COMMIT" and not a:
            if stack:
                stack = []
            else:
                out.append("NO TRANSACTION")
        else:
            out.append("ERR unknown command")
    print("\n".join(out))
main()
