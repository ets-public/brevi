import sys, json, re
accts = {}
order = []
def J(o):
    return json.dumps(o, separators=(',', ':'), ensure_ascii=False)
def acc(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}
def isint(x):
    return isinstance(x, int) and not isinstance(x, bool)
def err(c, m):
    return "%d %s" % (c, J({"error": m}))
def amt_ok(b):
    a = b.get("amount")
    return isint(a) and a > 0
def handle(line):
    parts = line.split(None, 2)
    if len(parts) < 2:
        return err(404, "not found")
    method, path = parts[0], parts[1]
    body = parts[2] if len(parts) > 2 else None
    route = None
    m = None
    if path == "/accounts" and method in ("POST", "GET"):
        route = method + " list"
    elif method == "GET" and (m := re.fullmatch(r"/accounts/(\d+)", path)):
        route = "get"
    elif method == "POST" and (m := re.fullmatch(r"/accounts/(\d+)/(deposit|withdraw)", path)):
        route = m.group(2)
    elif method == "POST" and path == "/transfer":
        route = "transfer"
    if route is None:
        return err(404, "not found")
    b = {}
    if method == "POST":
        if body is not None and body.strip():
            try:
                b = json.loads(body)
            except Exception:
                return err(400, "bad json")
            if not isinstance(b, dict):
                return err(400, "bad json")
    if route == "POST list":
        o = b.get("owner")
        if not isinstance(o, str) or o == "":
            return err(400, "owner required")
        a = {"id": len(order) + 1, "owner": o, "balance": 0}
        accts[a["id"]] = a
        order.append(a)
        return "201 " + J(acc(a))
    if route == "GET list":
        l = sorted(order, key=lambda a: (-a["balance"], a["id"]))
        return "200 " + J([acc(a) for a in l])
    if route == "get":
        a = accts.get(int(m.group(1)))
        if not a:
            return err(404, "no such account")
        return "200 " + J(acc(a))
    if route in ("deposit", "withdraw"):
        a = accts.get(int(m.group(1)))
        if not a:
            return err(404, "no such account")
        if not amt_ok(b):
            return err(400, "bad amount")
        n = b["amount"]
        if route == "withdraw":
            if n > a["balance"]:
                return err(409, "insufficient funds")
            a["balance"] -= n
        else:
            a["balance"] += n
        return "200 " + J(acc(a))
    f = b.get("from"); t = b.get("to")
    fa = accts.get(f) if isint(f) else None
    ta = accts.get(t) if isint(t) else None
    if not fa or not ta:
        return err(404, "no such account")
    if f == t:
        return err(400, "same account")
    if not amt_ok(b):
        return err(400, "bad amount")
    n = b["amount"]
    if n > fa["balance"]:
        return err(409, "insufficient funds")
    fa["balance"] -= n
    ta["balance"] += n
    return "200 " + J({"from": fa["balance"], "to": ta["balance"]})
for line in sys.stdin.read().split("\n"):
    line = line.strip()
    if line:
        print(handle(line))
