import sys, json, re
accts = {}
order = []
def J(o):
    return json.dumps(o, separators=(',', ':'))
def acc(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}
def okint(v):
    return isinstance(v, int) and not isinstance(v, bool) and v > 0
def handle(line):
    parts = line.split(' ', 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ''
    body = parts[2].strip() if len(parts) > 2 else ''
    route = None
    if method == 'POST' and path == '/accounts': route = ('create',)
    elif method == 'GET' and path == '/accounts': route = ('list',)
    elif method == 'POST' and path == '/transfer': route = ('transfer',)
    else:
        m = re.fullmatch(r'/accounts/(\d+)', path)
        if m and method == 'GET': route = ('get', int(m.group(1)))
        m = re.fullmatch(r'/accounts/(\d+)/(deposit|withdraw)', path)
        if m and method == 'POST': route = (m.group(2), int(m.group(1)))
    if route is None:
        return '404 ' + J({"error": "not found"})
    data = {}
    if route[0] in ('create', 'deposit', 'withdraw', 'transfer') and body:
        try:
            data = json.loads(body)
        except Exception:
            return '400 ' + J({"error": "bad json"})
        if not isinstance(data, dict):
            data = {}
    k = route[0]
    nf = '404 ' + J({"error": "no such account"})
    if k == 'create':
        o = data.get('owner')
        if not isinstance(o, str) or o == '':
            return '400 ' + J({"error": "owner required"})
        a = {"id": len(order) + 1, "owner": o, "balance": 0}
        accts[a["id"]] = a; order.append(a)
        return '201 ' + J(acc(a))
    if k == 'list':
        l = sorted(order, key=lambda a: (-a["balance"], a["id"]))
        return '200 ' + J([acc(a) for a in l])
    if k == 'get':
        a = accts.get(route[1])
        return '200 ' + J(acc(a)) if a else nf
    if k in ('deposit', 'withdraw'):
        a = accts.get(route[1])
        if not a: return nf
        n = data.get('amount')
        if not okint(n): return '400 ' + J({"error": "bad amount"})
        if k == 'deposit':
            a["balance"] += n
        else:
            if n > a["balance"]: return '409 ' + J({"error": "insufficient funds"})
            a["balance"] -= n
        return '200 ' + J(acc(a))
    f = data.get('from'); t = data.get('to')
    fa = accts.get(f) if isinstance(f, int) and not isinstance(f, bool) else None
    ta = accts.get(t) if isinstance(t, int) and not isinstance(t, bool) else None
    if not fa or not ta: return nf
    if fa is ta: return '400 ' + J({"error": "same account"})
    n = data.get('amount')
    if not okint(n): return '400 ' + J({"error": "bad amount"})
    if n > fa["balance"]: return '409 ' + J({"error": "insufficient funds"})
    fa["balance"] -= n; ta["balance"] += n
    return '200 ' + J({"from": fa["balance"], "to": ta["balance"]})
for line in sys.stdin.read().split('\n'):
    line = line.strip()
    if line:
        print(handle(line))
