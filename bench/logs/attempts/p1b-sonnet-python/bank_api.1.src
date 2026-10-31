import sys, json

accounts = []


def out(code, body):
    print("%d %s" % (code, json.dumps(body, separators=(",", ":"), ensure_ascii=False)))


def acc(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}


def isid(s):
    return s.isascii() and s.isdigit()


def getacc(i):
    if isinstance(i, int) and not isinstance(i, bool) and 1 <= i <= len(accounts):
        return accounts[i - 1]
    return None


def valid_amount(a):
    return isinstance(a, int) and not isinstance(a, bool) and a > 0


def handle(line):
    parts = line.split(" ", 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ""
    body = parts[2] if len(parts) > 2 else None
    segs = path.split("/")
    route = None
    if method == "POST" and path == "/accounts":
        route = "create"
    elif method == "GET" and path == "/accounts":
        route = "list"
    elif method == "GET" and len(segs) == 3 and segs[1] == "accounts" and isid(segs[2]):
        route = "get"
    elif method == "POST" and len(segs) == 4 and segs[1] == "accounts" and isid(segs[2]) and segs[3] in ("deposit", "withdraw"):
        route = segs[3]
    elif method == "POST" and path == "/transfer":
        route = "transfer"
    if route is None:
        return out(404, {"error": "not found"})
    data = {}
    if route in ("create", "deposit", "withdraw", "transfer"):
        try:
            if body is None:
                raise ValueError
            data = json.loads(body)
        except Exception:
            return out(400, {"error": "bad json"})
        if not isinstance(data, dict):
            data = {}
    if route == "create":
        o = data.get("owner")
        if not isinstance(o, str) or o == "":
            return out(400, {"error": "owner required"})
        a = {"id": len(accounts) + 1, "owner": o, "balance": 0}
        accounts.append(a)
        return out(201, acc(a))
    if route == "list":
        l = sorted(accounts, key=lambda a: (-a["balance"], a["id"]))
        return out(200, [acc(a) for a in l])
    if route == "get":
        a = getacc(int(segs[2]))
        if a is None:
            return out(404, {"error": "no such account"})
        return out(200, acc(a))
    if route in ("deposit", "withdraw"):
        a = getacc(int(segs[2]))
        if a is None:
            return out(404, {"error": "no such account"})
        n = data.get("amount")
        if not valid_amount(n):
            return out(400, {"error": "bad amount"})
        if route == "withdraw":
            if n > a["balance"]:
                return out(409, {"error": "insufficient funds"})
            a["balance"] -= n
        else:
            a["balance"] += n
        return out(200, acc(a))
    if route == "transfer":
        f = getacc(data.get("from"))
        t = getacc(data.get("to"))
        if f is None or t is None:
            return out(404, {"error": "no such account"})
        if f is t:
            return out(400, {"error": "same account"})
        n = data.get("amount")
        if not valid_amount(n):
            return out(400, {"error": "bad amount"})
        if n > f["balance"]:
            return out(409, {"error": "insufficient funds"})
        f["balance"] -= n
        t["balance"] += n
        return out(200, {"from": f["balance"], "to": t["balance"]})


for line in sys.stdin.read().split("\n"):
    line = line.strip()
    if not line:
        continue
    handle(line)
