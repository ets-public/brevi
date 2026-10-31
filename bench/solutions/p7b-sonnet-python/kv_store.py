import sys
kv = {}
stack = []
def setk(k, v):
    if stack and k not in stack[-1]:
        stack[-1][k] = kv.get(k)
    if v is None:
        kv.pop(k, None)
    else:
        kv[k] = v
def isint(s):
    try:
        int(s); return True
    except ValueError:
        return False
arity = {'SET': 2, 'GET': 1, 'DEL': 1, 'INCR': 2, 'COUNT': 1, 'KEYS': 1, 'BEGIN': 0, 'ROLLBACK': 0, 'COMMIT': 0}
for line in sys.stdin.read().split('\n'):
    p = line.split()
    if not p:
        continue
    c = p[0]; a = p[1:]
    if c not in arity or len(a) != arity[c]:
        print('ERR unknown command'); continue
    if c == 'SET':
        setk(a[0], a[1])
    elif c == 'GET':
        print(kv.get(a[0], 'NULL'))
    elif c == 'DEL':
        if a[0] in kv: setk(a[0], None)
        else: print('NOT FOUND')
    elif c == 'INCR':
        if not isint(a[1]):
            print('ERR unknown command'); continue
        cur = kv.get(a[0], '0')
        if not isint(cur):
            print('ERR not an integer'); continue
        n = int(cur) + int(a[1])
        setk(a[0], str(n)); print(n)
    elif c == 'COUNT':
        print(sum(1 for v in kv.values() if v == a[0]))
    elif c == 'KEYS':
        ks = sorted(k for k in kv if k.startswith(a[0]))
        print(' '.join(ks) if ks else '(empty)')
    elif c == 'BEGIN':
        stack.append({})
    elif c == 'ROLLBACK':
        if not stack: print('NO TRANSACTION')
        else:
            for k, v in stack.pop().items():
                if v is None: kv.pop(k, None)
                else: kv[k] = v
    elif c == 'COMMIT':
        if not stack: print('NO TRANSACTION')
        else: stack = []
