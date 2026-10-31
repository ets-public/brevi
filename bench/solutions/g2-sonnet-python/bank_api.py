import sys, json
accts = {}
def out(code, obj):
    print(code, json.dumps(obj, separators=(",", ":")))
def isint(x):
    return isinstance(x, int) and not isinstance(x, bool)
def view(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}
def amount_ok(b):
    n = b.get("amount")
    return isint(n) and n > 0
def handle(line):
    parts = line.split(" ", 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ""
    raw = parts[2].strip() if len(parts) > 2 else ""
    seg = [s for s in path.split("/")]
    route = None
    aid = None
    if seg[:1] == [""] and len(seg) >= 2:
        s = seg[1:]
        if s == ["accounts"] and method in ("POST", "GET"):
            route = method + " list"
        elif s == ["transfer"] and method == "POST":
            route = "transfer"
        elif len(s) >= 2 and s[0] == "accounts" and s[1].isascii() and s[1].isdigit():
            aid = int(s[1])
            if len(s) == 2 and method == "GET":
                route = "get"
            elif len(s) == 3 and method == "POST" and s[2] in ("deposit", "withdraw"):
                route = s[2]
    if route is None:
        return out(404, {"error": "not found"})
    body = {}
    if raw:
        try:
            body = json.loads(raw)
        except Exception:
            return out(400, {"error": "bad json"})
        if not isinstance(body, dict):
            return out(400, {"error": "bad json"})
    if route == "POST list":
        o = body.get("owner")
        if not isinstance(o, str) or not o:
            return out(400, {"error": "owner required"})
        a = {"id": len(accts) + 1, "owner": o, "balance": 0}
        accts[a["id"]] = a
        return out(201, view(a))
    if route == "GET list":
        l = sorted(accts.values(), key=lambda a: (-a["balance"], a["id"]))
        return out(200, [view(a) for a in l])
    if route == "get":
        if aid not in accts:
            return out(404, {"error": "no such account"})
        return out(200, view(accts[aid]))
    if route in ("deposit", "withdraw"):
        if aid not in accts:
            return out(404, {"error": "no such account"})
        if not amount_ok(body):
            return out(400, {"error": "bad amount"})
        a = accts[aid]
        n = body["amount"]
        if route == "withdraw":
            if n > a["balance"]:
                return out(409, {"error": "insufficient funds"})
            a["balance"] -= n
        else:
            a["balance"] += n
        return out(200, view(a))
    if route == "transfer":
        f, t = body.get("from"), body.get("to")
        if not isint(f) or f not in accts or not isint(t) or t not in accts:
            return out(404, {"error": "no such account"})
        if f == t:
            return out(400, {"error": "same account"})
        if not amount_ok(body):
            return out(400, {"error": "bad amount"})
        n = body["amount"]
        if n > accts[f]["balance"]:
            return out(409, {"error": "insufficient funds"})
        accts[f]["balance"] -= n
        accts[t]["balance"] += n
        return out(200, {"from": accts[f]["balance"], "to": accts[t]["balance"]})
for line in sys.stdin.read().split("\n"):
    line = line.strip()
    if line:
        handle(line)
