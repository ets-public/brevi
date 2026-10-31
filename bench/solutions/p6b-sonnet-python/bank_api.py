import sys, json
accts = []
def obj(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}
def out(code, body):
    print(code, json.dumps(body, separators=(',', ':')))
def find(s):
    if s.isdigit():
        i = int(s)
        if 1 <= i <= len(accts):
            return accts[i - 1]
    return None
def getid(v):
    if isinstance(v, int) and not isinstance(v, bool) and 1 <= v <= len(accts):
        return accts[v - 1]
    return None
def okamt(v):
    return isinstance(v, int) and not isinstance(v, bool) and v > 0
def handle(line):
    parts = line.split(' ', 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ''
    rest = parts[2] if len(parts) > 2 else None
    segs = path.strip('/').split('/') if path.startswith('/') else []
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
        return out(404, {"error": "not found"})
    body = {}
    if route in ('create', 'deposit', 'withdraw', 'transfer'):
        if rest is not None and rest.strip():
            try:
                body = json.loads(rest)
            except Exception:
                return out(400, {"error": "bad json"})
        if not isinstance(body, dict):
            body = {}
    if route == 'create':
        o = body.get("owner")
        if not isinstance(o, str) or not o:
            return out(400, {"error": "owner required"})
        a = {"id": len(accts) + 1, "owner": o, "balance": 0}
        accts.append(a)
        return out(201, obj(a))
    if route == 'list':
        return out(200, [obj(a) for a in sorted(accts, key=lambda a: (-a["balance"], a["id"]))])
    if route == 'get':
        a = find(segs[1])
        if not a:
            return out(404, {"error": "no such account"})
        return out(200, obj(a))
    if route in ('deposit', 'withdraw'):
        a = find(segs[1])
        if not a:
            return out(404, {"error": "no such account"})
        n = body.get("amount")
        if not okamt(n):
            return out(400, {"error": "bad amount"})
        if route == 'withdraw':
            if n > a["balance"]:
                return out(409, {"error": "insufficient funds"})
            a["balance"] -= n
        else:
            a["balance"] += n
        return out(200, obj(a))
    f = getid(body.get("from")); t = getid(body.get("to"))
    if not f or not t:
        return out(404, {"error": "no such account"})
    if f is t:
        return out(400, {"error": "same account"})
    n = body.get("amount")
    if not okamt(n):
        return out(400, {"error": "bad amount"})
    if n > f["balance"]:
        return out(409, {"error": "insufficient funds"})
    f["balance"] -= n; t["balance"] += n
    out(200, {"from": f["balance"], "to": t["balance"]})
for line in sys.stdin.read().split('\n'):
    line = line.strip()
    if line:
        handle(line)
