import sys, json, re

accts = {}
order = []


def dump(o):
    return json.dumps(o, separators=(',', ':'), ensure_ascii=False)


def acc(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}


def isint(v):
    return isinstance(v, int) and not isinstance(v, bool)


def handle(line):
    parts = line.split(" ", 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ""
    rest = parts[2] if len(parts) > 2 else None
    nf = (404, {"error": "not found"})
    route = None
    m = None
    if method == "POST" and path == "/accounts":
        route = "create"
    elif method == "GET" and path == "/accounts":
        route = "list"
    elif method == "POST" and path == "/transfer":
        route = "transfer"
    else:
        m = re.fullmatch(r'/accounts/(\d+)', path)
        if m and method == "GET":
            route = "get"
        else:
            m = re.fullmatch(r'/accounts/(\d+)/(deposit|withdraw)', path)
            if m and method == "POST":
                route = m.group(2)
    if route is None:
        return nf
    body = {}
    if route in ("create", "deposit", "withdraw", "transfer"):
        if rest is not None and rest.strip():
            try:
                body = json.loads(rest)
            except Exception:
                return 400, {"error": "bad json"}
        if not isinstance(body, dict):
            body = {}
    if route == "list":
        l = sorted(accts.values(), key=lambda a: (-a["balance"], a["id"]))
        return 200, [acc(a) for a in l]
    if route == "create":
        o = body.get("owner")
        if not isinstance(o, str) or o == "":
            return 400, {"error": "owner required"}
        a = {"id": len(accts) + 1, "owner": o, "balance": 0}
        accts[a["id"]] = a
        return 201, acc(a)
    if route == "get":
        a = accts.get(int(m.group(1)))
        if not a:
            return 404, {"error": "no such account"}
        return 200, acc(a)
    if route in ("deposit", "withdraw"):
        a = accts.get(int(m.group(1)))
        if not a:
            return 404, {"error": "no such account"}
        n = body.get("amount")
        if not isint(n) or n <= 0:
            return 400, {"error": "bad amount"}
        if route == "deposit":
            a["balance"] += n
        else:
            if n > a["balance"]:
                return 409, {"error": "insufficient funds"}
            a["balance"] -= n
        return 200, acc(a)
    # transfer
    f = body.get("from")
    t = body.get("to")
    fa = accts.get(f) if isint(f) else None
    ta = accts.get(t) if isint(t) else None
    if not fa or not ta:
        return 404, {"error": "no such account"}
    if fa is ta:
        return 400, {"error": "same account"}
    n = body.get("amount")
    if not isint(n) or n <= 0:
        return 400, {"error": "bad amount"}
    if n > fa["balance"]:
        return 409, {"error": "insufficient funds"}
    fa["balance"] -= n
    ta["balance"] += n
    return 200, {"from": fa["balance"], "to": ta["balance"]}


for line in sys.stdin.read().split("\n"):
    line = line.strip()
    if not line:
        continue
    code, obj = handle(line)
    print("%d %s" % (code, dump(obj)))
