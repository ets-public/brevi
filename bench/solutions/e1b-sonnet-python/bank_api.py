import sys, json
accts = {}
order = []
def J(x):
    return json.dumps(x, separators=(',', ':'))
def acc(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}
def amt_ok(b):
    v = b.get("amount")
    return isinstance(v, int) and not isinstance(v, bool) and v > 0
def getacc(v):
    if isinstance(v, int) and not isinstance(v, bool):
        return accts.get(v)
    return None
def handle(line):
    parts = line.split(' ', 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ''
    body = parts[2] if len(parts) > 2 else None
    segs = path.strip('/').split('/') if path.startswith('/') else None
    NF = (404, {"error": "not found"})
    if segs is None:
        return NF
    route = None
    if method == 'POST' and segs == ['accounts']:
        route = 'create'
    elif method == 'GET' and segs == ['accounts']:
        route = 'list'
    elif method == 'GET' and len(segs) == 2 and segs[0] == 'accounts' and segs[1].isdigit():
        route = 'get'
    elif method == 'POST' and len(segs) == 3 and segs[0] == 'accounts' and segs[1].isdigit() and segs[2] in ('deposit', 'withdraw'):
        route = segs[2]
    elif method == 'POST' and segs == ['transfer']:
        route = 'transfer'
    if route is None:
        return NF
    b = {}
    if route in ('create', 'deposit', 'withdraw', 'transfer'):
        try:
            b = json.loads(body) if body is not None else None
            if body is None:
                raise ValueError
        except Exception:
            return 400, {"error": "bad json"}
        if not isinstance(b, dict):
            b = {}
    if route == 'create':
        o = b.get("owner")
        if not isinstance(o, str) or not o:
            return 400, {"error": "owner required"}
        i = len(order) + 1
        a = {"id": i, "owner": o, "balance": 0}
        accts[i] = a
        order.append(a)
        return 201, acc(a)
    if route == 'list':
        l = sorted(order, key=lambda a: (-a["balance"], a["id"]))
        return 200, [acc(a) for a in l]
    if route == 'get':
        a = accts.get(int(segs[1]))
        if not a:
            return 404, {"error": "no such account"}
        return 200, acc(a)
    if route in ('deposit', 'withdraw'):
        a = accts.get(int(segs[1]))
        if not a:
            return 404, {"error": "no such account"}
        if not amt_ok(b):
            return 400, {"error": "bad amount"}
        n = b["amount"]
        if route == 'deposit':
            a["balance"] += n
        else:
            if n > a["balance"]:
                return 409, {"error": "insufficient funds"}
            a["balance"] -= n
        return 200, acc(a)
    f = getacc(b.get("from")); t = getacc(b.get("to"))
    if not f or not t:
        return 404, {"error": "no such account"}
    if f is t:
        return 400, {"error": "same account"}
    if not amt_ok(b):
        return 400, {"error": "bad amount"}
    n = b["amount"]
    if n > f["balance"]:
        return 409, {"error": "insufficient funds"}
    f["balance"] -= n
    t["balance"] += n
    return 200, {"from": f["balance"], "to": t["balance"]}
for line in sys.stdin.read().split('\n'):
    line = line.strip()
    if not line:
        continue
    c, r = handle(line)
    print("%d %s" % (c, J(r)))
