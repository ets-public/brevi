import sys
data = {}
stack = []
def setk(k, v):
    if stack and k not in stack[-1]:
        stack[-1][k] = data.get(k)
    if v is None:
        data.pop(k, None)
    else:
        data[k] = v
for line in sys.stdin.read().split('\n'):
    p = line.split()
    if not p:
        continue
    c = p[0]
    a = p[1:]
    if c == 'SET' and len(a) == 2:
        setk(a[0], a[1])
    elif c == 'GET' and len(a) == 1:
        print(data.get(a[0], 'NULL'))
    elif c == 'DEL' and len(a) == 1:
        if a[0] in data:
            setk(a[0], None)
        else:
            print('NOT FOUND')
    elif c == 'INCR' and len(a) == 2:
        try:
            n = int(a[1])
            cur = int(data.get(a[0], '0'))
        except ValueError:
            print('ERR not an integer')
            continue
        setk(a[0], str(cur + n))
        print(cur + n)
    elif c == 'COUNT' and len(a) == 1:
        print(sum(1 for v in data.values() if v == a[0]))
    elif c == 'KEYS' and len(a) == 1:
        ks = sorted(k for k in data if k.startswith(a[0]))
        print(' '.join(ks) if ks else '(empty)')
    elif c == 'BEGIN' and not a:
        stack.append({})
    elif c == 'ROLLBACK' and not a:
        if not stack:
            print('NO TRANSACTION')
        else:
            for k, v in stack.pop().items():
                if v is None:
                    data.pop(k, None)
                else:
                    data[k] = v
    elif c == 'COMMIT' and not a:
        if not stack:
            print('NO TRANSACTION')
        else:
            stack = []
    else:
        print('ERR unknown command')
