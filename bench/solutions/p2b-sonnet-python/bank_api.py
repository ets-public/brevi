import sys, json

accts = {}
order = []

def out(code, body):
    print(code, json.dumps(body, separators=(',', ':')))

def acc(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}

def isint(v):
    return isinstance(v, int) and not isinstance(v, bool)

def good_amount(v):
    return isint(v) and v > 0

def handle(line):
    parts = line.split(None, 2)
    if len(parts) < 2:
        return out(404, {"error": "not found"})
    method, path = parts[0], parts[1]
    rest = parts[2] if len(parts) > 2 else None
    segs = path.strip('/').split('/') if path.startswith('/') else None
    if segs is None:
        return out(404, {"error": "not found"})
    route = None
    aid = None
    if method == 'POST' and segs == ['accounts']:
        route = 'create'
    elif method == 'GET' and segs == ['accounts']:
        route = 'list'
    elif method == 'POST' and segs == ['transfer']:
        route = 'transfer'
    elif len(segs) >= 2 and segs[0] == 'accounts' and segs[1].isdigit() and segs[1].isascii():
        aid = int(segs[1])
        if method == 'GET' and len(segs) == 2:
            route = 'get'
        elif method == 'POST' and len(segs) == 3 and segs[2] in ('deposit', 'withdraw'):
            route = segs[2]
    if route is None:
        return out(404, {"error": "not found"})
    body = {}
    if method == 'POST':
        if rest is not None and rest.strip():
            try:
                body = json.loads(rest)
            except Exception:
                return out(400, {"error": "bad json"})
        if not isinstance(body, dict):
            body = {}
    if route == 'create':
        o = body.get("owner")
        if not isinstance(o, str) or o == "":
            return out(400, {"error": "owner required"})
        a = {"id": len(order) + 1, "owner": o, "balance": 0}
        accts[a["id"]] = a
        order.append(a)
        return out(201, acc(a))
    if route == 'list':
        l = sorted(order, key=lambda a: (-a["balance"], a["id"]))
        return out(200, [acc(a) for a in l])
    if route == 'get':
        if aid not in accts:
            return out(404, {"error": "no such account"})
        return out(200, acc(accts[aid]))
    if route in ('deposit', 'withdraw'):
        if aid not in accts:
            return out(404, {"error": "no such account"})
        n = body.get("amount")
        if not good_amount(n):
            return out(400, {"error": "bad amount"})
        a = accts[aid]
        if route == 'withdraw':
            if n > a["balance"]:
                return out(409, {"error": "insufficient funds"})
            a["balance"] -= n
        else:
            a["balance"] += n
        return out(200, acc(a))
    if route == 'transfer':
        f, t = body.get("from"), body.get("to")
        if not isint(f) or f not in accts or not isint(t) or t not in accts:
            return out(404, {"error": "no such account"})
        if f == t:
            return out(400, {"error": "same account"})
        n = body.get("amount")
        if not good_amount(n):
            return out(400, {"error": "bad amount"})
        if n > accts[f]["balance"]:
            return out(409, {"error": "insufficient funds"})
        accts[f]["balance"] -= n
        accts[t]["balance"] += n
        return out(200, {"from": accts[f]["balance"], "to": accts[t]["balance"]})

for line in sys.stdin.read().split('\n'):
    line = line.strip()
    if line:
        handle(line)
