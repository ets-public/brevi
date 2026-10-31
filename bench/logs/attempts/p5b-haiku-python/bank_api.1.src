import json
import re
import sys

ID_RE = re.compile(r"[0-9]+")

accounts = []  # list of dicts in creation order; id = index + 1


def out(status, body):
    return f"{status} {json.dumps(body, separators=(',', ':'))}"


def not_found():
    return out(404, {"error": "not found"})


def no_account():
    return out(404, {"error": "no such account"})


def bad_amount():
    return out(400, {"error": "bad amount"})


def valid_amount(v):
    return type(v) is int and v > 0


def find(acc_id):
    if 1 <= acc_id <= len(accounts):
        return accounts[acc_id - 1]
    return None


def account_view(acc):
    return {"id": acc["id"], "owner": acc["owner"], "balance": acc["balance"]}


def handle(method, path, body_text):
    parts = path.split("/")
    # parts[0] is '' for absolute paths
    segs = parts[1:]

    route = None
    if segs == ["accounts"]:
        if method == "POST":
            route = "create"
        elif method == "GET":
            route = "list"
    elif len(segs) == 2 and segs[0] == "accounts" and ID_RE.fullmatch(segs[1]):
        if method == "GET":
            route = "get"
    elif len(segs) == 3 and segs[0] == "accounts" and ID_RE.fullmatch(segs[1]):
        if method == "POST" and segs[2] in ("deposit", "withdraw"):
            route = segs[2]
    elif segs == ["transfer"]:
        if method == "POST":
            route = "transfer"

    if route is None:
        return not_found()

    body = None
    if route in ("create", "deposit", "withdraw", "transfer"):
        text = body_text.strip() if body_text is not None else ""
        if text == "":
            body = {}
        else:
            try:
                body = json.loads(text)
            except ValueError:
                return out(400, {"error": "bad json"})
            if not isinstance(body, dict):
                body = {}

    if route == "create":
        owner = body.get("owner")
        if not isinstance(owner, str) or owner == "":
            return out(400, {"error": "owner required"})
        acc = {"id": len(accounts) + 1, "owner": owner, "balance": 0}
        accounts.append(acc)
        return out(201, account_view(acc))

    if route == "list":
        ordered = sorted(accounts, key=lambda a: (-a["balance"], a["id"]))
        return out(200, [account_view(a) for a in ordered])

    if route == "get":
        acc = find(int(segs[1]))
        if acc is None:
            return no_account()
        return out(200, account_view(acc))

    if route in ("deposit", "withdraw"):
        acc = find(int(segs[1]))
        if acc is None:
            return no_account()
        amount = body.get("amount")
        if not valid_amount(amount):
            return bad_amount()
        if route == "deposit":
            acc["balance"] += amount
        else:
            if amount > acc["balance"]:
                return out(409, {"error": "insufficient funds"})
            acc["balance"] -= amount
        return out(200, account_view(acc))

    if route == "transfer":
        src_id = body.get("from")
        dst_id = body.get("to")
        src = find(src_id) if type(src_id) is int else None
        dst = find(dst_id) if type(dst_id) is int else None
        if src is None or dst is None:
            return no_account()
        if src is dst:
            return out(400, {"error": "same account"})
        amount = body.get("amount")
        if not valid_amount(amount):
            return bad_amount()
        if amount > src["balance"]:
            return out(409, {"error": "insufficient funds"})
        src["balance"] -= amount
        dst["balance"] += amount
        return out(200, {"from": src["balance"], "to": dst["balance"]})

    return not_found()


def main():
    results = []
    for raw in sys.stdin.read().splitlines():
        line = raw.rstrip("\r")
        if line.strip() == "":
            continue
        parts = line.split(" ", 2)
        method = parts[0]
        path = parts[1] if len(parts) > 1 else ""
        body_text = parts[2] if len(parts) > 2 else None
        results.append(handle(method, path, body_text))
    sys.stdout.write("\n".join(results) + ("\n" if results else ""))


if __name__ == "__main__":
    main()
