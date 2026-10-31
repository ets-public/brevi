import json
import sys


def dump(obj):
    return json.dumps(obj, separators=(",", ":"), ensure_ascii=False)


def resp(status, body):
    return f"{status} {dump(body)}"


NOT_FOUND = (404, {"error": "not found"})
NO_ACCOUNT = (404, {"error": "no such account"})
BAD_JSON = (400, {"error": "bad json"})
BAD_AMOUNT = (400, {"error": "bad amount"})


def is_pos_int(v):
    return isinstance(v, int) and not isinstance(v, bool) and v > 0


def main():
    accounts = {}  # id -> {"id", "owner", "balance"}
    next_id = [1]
    out = []

    def acct_obj(a):
        return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}

    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        head, _, body_text = line.partition(" ")
        method = head
        path = body_text.strip()
        body_src = None
        if " " in path:
            path, _, body_src = path.partition(" ")
            body_src = body_src.strip()

        segs = [s for s in path.split("/")]
        # Route check.
        route = None
        if path == "/accounts" and method == "POST":
            route = "create"
        elif path == "/accounts" and method == "GET":
            route = "list"
        elif path == "/transfer" and method == "POST":
            route = "transfer"
        elif len(segs) == 3 and segs[0] == "" and segs[1] == "accounts" and method == "GET":
            route = "get"
        elif (len(segs) == 4 and segs[0] == "" and segs[1] == "accounts"
              and segs[3] in ("deposit", "withdraw") and method == "POST"):
            route = segs[3]
        if route is None:
            out.append(resp(*NOT_FOUND))
            continue

        aid = None
        if route in ("get", "deposit", "withdraw"):
            if not segs[2].isdigit() or not segs[2].isascii():
                out.append(resp(*NOT_FOUND))
                continue
            aid = int(segs[2])

        # JSON body check (for routes that take a body).
        body = None
        if route in ("create", "deposit", "withdraw", "transfer"):
            if body_src is None or body_src == "":
                body = {}
            else:
                try:
                    body = json.loads(body_src)
                except (ValueError, TypeError):
                    out.append(resp(*BAD_JSON))
                    continue
                if not isinstance(body, dict):
                    out.append(resp(*BAD_JSON))
                    continue

        if route == "create":
            owner = body.get("owner")
            if not isinstance(owner, str) or owner.strip() == "":
                out.append(resp(400, {"error": "owner required"}))
                continue
            aid_new = next_id[0]
            next_id[0] += 1
            accounts[aid_new] = {"id": aid_new, "owner": owner, "balance": 0}
            out.append(resp(201, acct_obj(accounts[aid_new])))

        elif route == "list":
            items = sorted(accounts.values(), key=lambda a: (-a["balance"], a["id"]))
            out.append("200 " + dump([acct_obj(a) for a in items]))

        elif route == "get":
            if aid not in accounts:
                out.append(resp(*NO_ACCOUNT))
            else:
                out.append(resp(200, acct_obj(accounts[aid])))

        elif route in ("deposit", "withdraw"):
            if aid not in accounts:
                out.append(resp(*NO_ACCOUNT))
                continue
            amount = body.get("amount")
            if not is_pos_int(amount):
                out.append(resp(*BAD_AMOUNT))
                continue
            a = accounts[aid]
            if route == "deposit":
                a["balance"] += amount
            else:
                if amount > a["balance"]:
                    out.append(resp(409, {"error": "insufficient funds"}))
                    continue
                a["balance"] -= amount
            out.append(resp(200, acct_obj(a)))

        elif route == "transfer":
            src = body.get("from")
            dst = body.get("to")
            if not (isinstance(src, int) and not isinstance(src, bool) and src in accounts):
                out.append(resp(*NO_ACCOUNT))
                continue
            if not (isinstance(dst, int) and not isinstance(dst, bool) and dst in accounts):
                out.append(resp(*NO_ACCOUNT))
                continue
            if src == dst:
                out.append(resp(400, {"error": "same account"}))
                continue
            amount = body.get("amount")
            if not is_pos_int(amount):
                out.append(resp(*BAD_AMOUNT))
                continue
            a, b = accounts[src], accounts[dst]
            if amount > a["balance"]:
                out.append(resp(409, {"error": "insufficient funds"}))
                continue
            a["balance"] -= amount
            b["balance"] += amount
            out.append(resp(200, {"from": a["balance"], "to": b["balance"]}))

    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
