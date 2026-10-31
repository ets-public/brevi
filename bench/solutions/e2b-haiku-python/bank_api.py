import json
import re
import sys

accounts = []  # index i holds account id i + 1
ACC_RE = re.compile(r'^/accounts/(\d+)$')
DEP_RE = re.compile(r'^/accounts/(\d+)/deposit$')
WD_RE = re.compile(r'^/accounts/(\d+)/withdraw$')


def out(status, obj):
    return f"{status} {json.dumps(obj, separators=(',', ':'))}"


def not_found_acc():
    return out(404, {"error": "no such account"})


def get_account(id_str):
    idx = int(id_str) - 1
    if 0 <= idx < len(accounts):
        return accounts[idx]
    return None


def account_obj(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}


def valid_amount(v):
    return type(v) is int and v > 0


def route(method, path):
    if method == "POST" and path == "/accounts":
        return "create"
    if method == "GET" and path == "/accounts":
        return "list"
    if method == "POST" and path == "/transfer":
        return "transfer"
    if method == "GET" and ACC_RE.match(path):
        return "get"
    if method == "POST" and DEP_RE.match(path):
        return "deposit"
    if method == "POST" and WD_RE.match(path):
        return "withdraw"
    return None


def handle(method, path, body_text):
    r = route(method, path)
    if r is None:
        return out(404, {"error": "not found"})

    body = {}
    if body_text is not None:
        try:
            body = json.loads(body_text)
        except ValueError:
            return out(400, {"error": "bad json"})
    if not isinstance(body, dict):
        body = {}

    if r == "create":
        owner = body.get("owner")
        if not isinstance(owner, str) or owner.strip() == "":
            return out(400, {"error": "owner required"})
        acc = {"id": len(accounts) + 1, "owner": owner, "balance": 0}
        accounts.append(acc)
        return out(201, account_obj(acc))

    if r == "list":
        ordered = sorted(accounts, key=lambda a: (-a["balance"], a["id"]))
        return out(200, [account_obj(a) for a in ordered])

    if r == "get":
        m = ACC_RE.match(path)
        acc = get_account(m.group(1))
        if acc is None:
            return not_found_acc()
        return out(200, account_obj(acc))

    if r in ("deposit", "withdraw"):
        m = DEP_RE.match(path) if r == "deposit" else WD_RE.match(path)
        acc = get_account(m.group(1))
        if acc is None:
            return not_found_acc()
        amount = body.get("amount")
        if not valid_amount(amount):
            return out(400, {"error": "bad amount"})
        if r == "deposit":
            acc["balance"] += amount
        else:
            if amount > acc["balance"]:
                return out(409, {"error": "insufficient funds"})
            acc["balance"] -= amount
        return out(200, account_obj(acc))

    if r == "transfer":
        src_id = body.get("from")
        dst_id = body.get("to")
        src = get_account(str(src_id)) if type(src_id) is int and src_id >= 1 else None
        dst = get_account(str(dst_id)) if type(dst_id) is int and dst_id >= 1 else None
        if src is None or dst is None:
            return not_found_acc()
        if src is dst:
            return out(400, {"error": "same account"})
        amount = body.get("amount")
        if not valid_amount(amount):
            return out(400, {"error": "bad amount"})
        if amount > src["balance"]:
            return out(409, {"error": "insufficient funds"})
        src["balance"] -= amount
        dst["balance"] += amount
        return out(200, {"from": src["balance"], "to": dst["balance"]})

    return out(404, {"error": "not found"})


def main():
    for raw in sys.stdin.read().splitlines():
        line = raw.rstrip("\r")
        if line.strip() == "":
            continue
        parts = line.split(" ", 2)
        method = parts[0]
        path = parts[1] if len(parts) > 1 else ""
        body_text = parts[2] if len(parts) > 2 else None
        if body_text is not None and body_text.strip() == "":
            body_text = None
        print(handle(method, path, body_text))


if __name__ == "__main__":
    main()
