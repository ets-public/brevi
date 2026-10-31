import json
import re
import sys

accounts = {}
next_id = 1


def out(status, obj):
    print(f"{status} {json.dumps(obj, separators=(',', ':'))}")


def acct_json(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}


def is_amount(v):
    return type(v) is int and v > 0


def find(sid):
    # sid is a digit string from the route
    return accounts.get(int(sid))


def lookup_id(v):
    if type(v) is int and v in accounts:
        return accounts[v]
    return None


def handle(method, path, body):
    global next_id
    # Route matching
    m_acc = re.fullmatch(r"/accounts/([0-9]+)", path)
    m_dep = re.fullmatch(r"/accounts/([0-9]+)/deposit", path)
    m_wd = re.fullmatch(r"/accounts/([0-9]+)/withdraw", path)
    if path == "/accounts" and method == "POST":
        route = "create"
    elif path == "/accounts" and method == "GET":
        route = "list"
    elif m_acc and method == "GET":
        route = "get"
    elif m_dep and method == "POST":
        route = "deposit"
    elif m_wd and method == "POST":
        route = "withdraw"
    elif path == "/transfer" and method == "POST":
        route = "transfer"
    else:
        out(404, {"error": "not found"})
        return

    # JSON body
    data = {}
    if body is not None:
        try:
            parsed = json.loads(body)
        except ValueError:
            out(400, {"error": "bad json"})
            return
        data = parsed if isinstance(parsed, dict) else {}

    if route == "create":
        owner = data.get("owner")
        if not isinstance(owner, str) or owner == "":
            out(400, {"error": "owner required"})
            return
        a = {"id": next_id, "owner": owner, "balance": 0}
        accounts[next_id] = a
        next_id += 1
        out(201, acct_json(a))
    elif route == "list":
        items = sorted(accounts.values(), key=lambda a: (-a["balance"], a["id"]))
        print("200 [" + ",".join(json.dumps(acct_json(a), separators=(',', ':')) for a in items) + "]")
    elif route == "get":
        a = find(m_acc.group(1))
        if a is None:
            out(404, {"error": "no such account"})
        else:
            out(200, acct_json(a))
    elif route in ("deposit", "withdraw"):
        sid = (m_dep or m_wd).group(1)
        a = find(sid)
        if a is None:
            out(404, {"error": "no such account"})
            return
        amt = data.get("amount")
        if not is_amount(amt):
            out(400, {"error": "bad amount"})
            return
        if route == "deposit":
            a["balance"] += amt
        else:
            if amt > a["balance"]:
                out(409, {"error": "insufficient funds"})
                return
            a["balance"] -= amt
        out(200, acct_json(a))
    elif route == "transfer":
        src = lookup_id(data.get("from"))
        dst = lookup_id(data.get("to"))
        if src is None or dst is None:
            out(404, {"error": "no such account"})
            return
        if src is dst:
            out(400, {"error": "same account"})
            return
        amt = data.get("amount")
        if not is_amount(amt):
            out(400, {"error": "bad amount"})
            return
        if amt > src["balance"]:
            out(409, {"error": "insufficient funds"})
            return
        src["balance"] -= amt
        dst["balance"] += amt
        out(200, {"from": src["balance"], "to": dst["balance"]})


for raw in sys.stdin:
    line = raw.rstrip("\r\n")
    if line.strip() == "":
        continue
    parts = line.split(" ", 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ""
    body = parts[2] if len(parts) > 2 else None
    handle(method, path, body)
