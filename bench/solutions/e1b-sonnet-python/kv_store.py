import sys
store = {}
stack = []
MISSING = object()
def setk(k, v):
    if stack:
        stack[-1].append((k, store.get(k, MISSING)))
    if v is MISSING:
        store.pop(k, None)
    else:
        store[k] = v
out = []
for line in sys.stdin.read().split('\n'):
    p = line.split()
    if not p:
        continue
    c = p[0]; a = p[1:]
    if c == 'SET' and len(a) == 2:
        setk(a[0], a[1])
    elif c == 'GET' and len(a) == 1:
        out.append(store.get(a[0], 'NULL'))
    elif c == 'DEL' and len(a) == 1:
        if a[0] in store:
            setk(a[0], MISSING)
        else:
            out.append('NOT FOUND')
    elif c == 'INCR' and len(a) == 2:
        try:
            n = int(a[1]); cur = int(store.get(a[0], '0'))
        except ValueError:
            out.append('ERR not an integer'); continue
        setk(a[0], str(cur + n))
        out.append(str(cur + n))
    elif c == 'COUNT' and len(a) == 1:
        out.append(str(sum(1 for v in store.values() if v == a[0])))
    elif c == 'KEYS' and len(a) == 1:
        ks = sorted(k for k in store if k.startswith(a[0]))
        out.append(' '.join(ks) if ks else '(empty)')
    elif c == 'BEGIN' and not a:
        stack.append([])
    elif c == 'ROLLBACK' and not a:
        if not stack:
            out.append('NO TRANSACTION')
        else:
            log = stack.pop()
            for k, old in reversed(log):
                if old is MISSING:
                    store.pop(k, None)
                else:
                    store[k] = old
    elif c == 'COMMIT' and not a:
        if not stack:
            out.append('NO TRANSACTION')
        else:
            stack.clear()
    else:
        out.append('ERR unknown command')
if out:
    print('\n'.join(out))
