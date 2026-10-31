import sys, json

accounts = {}
order = []

def j(o):
    return json.dumps(o, separators=(",", ":"))

def acct(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}

def isint(x):
    return isinstance(x, int) and not isinstance(x, bool)

def find(x):
    if isint(x):
        return accounts.get(x)
    return None

def handle(line):
    parts = line.split(" ", 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ""
    body = parts[2].strip() if len(parts) > 2 else None
    segs = [s for s in path.split("/")]
    route = None
    aid = None
    if not path.startswith("/"):
        return "404", {"error": "not found"}
    segs = path[1:].split("/")
    if method == "POST" and segs == ["accounts"]:
        route = "create"
    elif method == "GET" and segs == ["accounts"]:
        route = "list"
    elif method == "POST" and segs == ["transfer"]:
        route = "transfer"
    elif len(segs) >= 2 and segs[0] == "accounts" and segs[1].isdigit() and segs[1].isascii():
        aid = int(segs[1])
        if method == "GET" and len(segs) == 2:
            route = "get"
        elif method == "POST" and len(segs) == 3 and segs[2] in ("deposit", "withdraw"):
            route = segs[2]
    if route is None:
        return "404", {"error": "not found"}
    data = {}
    if route in ("create", "deposit", "withdraw", "transfer"):
        if body:
            try:
                data = json.loads(body)
            except Exception:
                return "400", {"error": "bad json"}
            if not isinstance(data, dict):
                data = {}
    if route == "create":
        o = data.get("owner")
        if not isinstance(o, str) or o == "":
            return "400", {"error": "owner required"}
        a = {"id": len(order) + 1, "owner": o, "balance": 0}
        accounts[a["id"]] = a
        order.append(a)
        return "201", acct(a)
    if route == "list":
        l = sorted(order, key=lambda a: (-a["balance"], a["id"]))
        return "200", [acct(a) for a in l]
    if route == "get":
        a = accounts.get(aid)
        if not a:
            return "404", {"error": "no such account"}
        return "200", acct(a)
    if route in ("deposit", "withdraw"):
        a = accounts.get(aid)
        if not a:
            return "404", {"error": "no such account"}
        n = data.get("amount")
        if not isint(n) or n <= 0:
            return "400", {"error": "bad amount"}
        if route == "withdraw":
            if n > a["balance"]:
                return "409", {"error": "insufficient funds"}
            a["balance"] -= n
        else:
            a["balance"] += n
        return "200", acct(a)
    if route == "transfer":
        f = find(data.get("from"))
        t = find(data.get("to"))
        if not f or not t:
            return "404", {"error": "no such account"}
        if f is t:
            return "400", {"error": "same account"}
        n = data.get("amount")
        if not isint(n) or n <= 0:
            return "400", {"error": "bad amount"}
        if n > f["balance"]:
            return "409", {"error": "insufficient funds"}
        f["balance"] -= n
        t["balance"] += n
        return "200", {"from": f["balance"], "to": t["balance"]}

for line in sys.stdin.read().split("\n"):
    line = line.strip()
    if not line:
        continue
    s, o = handle(line)
    print(s + " " + j(o))
