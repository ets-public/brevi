import json
import re
import sys

NOT_FOUND = (404, {"error": "not found"})
NO_ACCOUNT = (404, {"error": "no such account"})
BAD_JSON = (400, {"error": "bad json"})
BAD_AMOUNT = (400, {"error": "bad amount"})

accounts = []  # index i holds the account with id i + 1
ID_RE = re.compile(r"[0-9]+")


def account_obj(acc):
    return {"id": acc["id"], "owner": acc["owner"], "balance": acc["balance"]}


def get_account(raw_id):
    if not isinstance(raw_id, int) or isinstance(raw_id, bool):
        return None
    if 1 <= raw_id <= len(accounts):
        return accounts[raw_id - 1]
    return None


def is_pos_int(v):
    return type(v) is int and v > 0


def parse_body(body):
    try:
        data = json.loads(body)
    except ValueError:
        return None, True
    if not isinstance(data, dict):
        data = {}
    return data, False


def handle(method, path, body):
    parts = path.split("/")

    if method == "POST" and path == "/accounts":
        data, bad = parse_body(body)
        if bad:
            return BAD_JSON
        owner = data.get("owner")
        if type(owner) is not str or owner == "":
            return 400, {"error": "owner required"}
        acc = {"id": len(accounts) + 1, "owner": owner, "balance": 0}
        accounts.append(acc)
        return 201, account_obj(acc)

    if method == "GET" and path == "/accounts":
        ordered = sorted(accounts, key=lambda a: (-a["balance"], a["id"]))
        return 200, [account_obj(a) for a in ordered]

    if method == "POST" and path == "/transfer":
        data, bad = parse_body(body)
        if bad:
            return BAD_JSON
        src = get_account(data.get("from"))
        dst = get_account(data.get("to"))
        if src is None or dst is None:
            return NO_ACCOUNT
        if src is dst:
            return 400, {"error": "same account"}
        amount = data.get("amount")
        if not is_pos_int(amount):
            return BAD_AMOUNT
        if amount > src["balance"]:
            return 409, {"error": "insufficient funds"}
        src["balance"] -= amount
        dst["balance"] += amount
        return 200, {"from": src["balance"], "to": dst["balance"]}

    if len(parts) >= 3 and parts[0] == "" and parts[1] == "accounts":
        if not ID_RE.fullmatch(parts[2]):
            return NOT_FOUND
        raw_id = int(parts[2])

        if len(parts) == 3 and method == "GET":
            acc = get_account(raw_id)
            if acc is None:
                return NO_ACCOUNT
            return 200, account_obj(acc)

        if len(parts) == 4 and method == "POST" and parts[3] in ("deposit", "withdraw"):
            data, bad = parse_body(body)
            if bad:
                return BAD_JSON
            acc = get_account(raw_id)
            if acc is None:
                return NO_ACCOUNT
            amount = data.get("amount")
            if not is_pos_int(amount):
                return BAD_AMOUNT
            if parts[3] == "deposit":
                acc["balance"] += amount
            else:
                if amount > acc["balance"]:
                    return 409, {"error": "insufficient funds"}
                acc["balance"] -= amount
            return 200, account_obj(acc)

    return NOT_FOUND


def main():
    out = []
    for raw in sys.stdin.read().splitlines():
        line = raw.rstrip("\r")
        if not line.strip():
            continue
        head, _, body = line.partition(" ")
        if " " in body:
            path, _, body = body.partition(" ")
        else:
            path, body = body, ""
        method = head
        status, payload = handle(method, path, body)
        out.append(f"{status} {json.dumps(payload, separators=(',', ':'))}")
    sys.stdout.write("\n".join(out) + ("\n" if out else ""))


if __name__ == "__main__":
    main()
