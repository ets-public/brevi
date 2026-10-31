import sys, json, re

accts = {}
order = []

def J(o):
    return json.dumps(o, separators=(',', ':'))

def view(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}

def isint(x):
    return isinstance(x, int) and not isinstance(x, bool)

def parse_body(s):
    try:
        b = json.loads(s)
    except Exception:
        return None
    return b if isinstance(b, dict) else None

def handle(line):
    parts = line.split(' ', 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ''
    body = parts[2].strip() if len(parts) > 2 else ''
    NF = (404, {"error": "not found"})
    BJ = (400, {"error": "bad json"})
    m = None
    if method == 'POST' and path == '/accounts':
        b = parse_body(body)
        if b is None:
            return BJ
        o = b.get('owner')
        if not isinstance(o, str) or not o:
            return 400, {"error": "owner required"}
        a = {"id": len(order) + 1, "owner": o, "balance": 0}
        accts[a["id"]] = a
        order.append(a)
        return 201, view(a)
    if method == 'GET' and path == '/accounts':
        l = sorted(order, key=lambda a: (-a["balance"], a["id"]))
        return 200, [view(a) for a in l]
    if method == 'GET':
        m = re.fullmatch(r'/accounts/(\d+)', path)
        if m:
            a = accts.get(int(m.group(1)))
            if not a:
                return 404, {"error": "no such account"}
            return 200, view(a)
        return NF
    if method == 'POST' and path == '/transfer':
        b = parse_body(body)
        if b is None:
            return BJ
        f = b.get('from'); t = b.get('to'); n = b.get('amount')
        fa = accts.get(f) if isint(f) else None
        ta = accts.get(t) if isint(t) else None
        if not fa or not ta:
            return 404, {"error": "no such account"}
        if fa is ta:
            return 400, {"error": "same account"}
        if not isint(n) or n <= 0:
            return 400, {"error": "bad amount"}
        if n > fa["balance"]:
            return 409, {"error": "insufficient funds"}
        fa["balance"] -= n
        ta["balance"] += n
        return 200, {"from": fa["balance"], "to": ta["balance"]}
    if method == 'POST':
        m = re.fullmatch(r'/accounts/(\d+)/(deposit|withdraw)', path)
        if m:
            b = parse_body(body)
            if b is None:
                return BJ
            a = accts.get(int(m.group(1)))
            if not a:
                return 404, {"error": "no such account"}
            n = b.get('amount')
            if not isint(n) or n <= 0:
                return 400, {"error": "bad amount"}
            if m.group(2) == 'deposit':
                a["balance"] += n
            else:
                if n > a["balance"]:
                    return 409, {"error": "insufficient funds"}
                a["balance"] -= n
            return 200, view(a)
    return NF

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    s, o = handle(line)
    print(s, J(o))
