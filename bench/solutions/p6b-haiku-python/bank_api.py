import sys
import json
import re

accounts = {}  # id -> {"id", "owner", "balance"}
next_id = [1]
BAD = object()


def out(status, obj):
    return f"{status} {json.dumps(obj, separators=(',', ':'), ensure_ascii=False)}"


def is_int(v):
    return isinstance(v, int) and not isinstance(v, bool)


def parse_body(body_text):
    if body_text is None:
        return BAD
    try:
        return json.loads(body_text)
    except (ValueError, TypeError):
        return BAD


def find(aid):
    if is_int(aid) and aid in accounts:
        return accounts[aid]
    return None


def handle(method, path, body_text):
    if method == 'GET' and path == '/accounts':
        lst = sorted(accounts.values(), key=lambda a: (-a['balance'], a['id']))
        return out(200, lst)

    if method == 'POST' and path == '/accounts':
        body = parse_body(body_text)
        if body is BAD:
            return out(400, {"error": "bad json"})
        owner = body.get('owner') if isinstance(body, dict) else None
        if not isinstance(owner, str) or owner == '':
            return out(400, {"error": "owner required"})
        aid = next_id[0]
        next_id[0] += 1
        acc = {"id": aid, "owner": owner, "balance": 0}
        accounts[aid] = acc
        return out(201, acc)

    if method == 'POST' and path == '/transfer':
        body = parse_body(body_text)
        if body is BAD:
            return out(400, {"error": "bad json"})
        if not isinstance(body, dict):
            body = {}
        src = find(body.get('from'))
        dst = find(body.get('to'))
        if src is None or dst is None:
            return out(404, {"error": "no such account"})
        if src['id'] == dst['id']:
            return out(400, {"error": "same account"})
        amount = body.get('amount')
        if not (is_int(amount) and amount > 0):
            return out(400, {"error": "bad amount"})
        if amount > src['balance']:
            return out(409, {"error": "insufficient funds"})
        src['balance'] -= amount
        dst['balance'] += amount
        return out(200, {"from": src['balance'], "to": dst['balance']})

    if method == 'GET':
        m = re.fullmatch(r'/accounts/(\d+)', path)
        if m:
            acc = find(int(m.group(1)))
            if acc is None:
                return out(404, {"error": "no such account"})
            return out(200, acc)

    if method == 'POST':
        m = re.fullmatch(r'/accounts/(\d+)/(deposit|withdraw)', path)
        if m:
            body = parse_body(body_text)
            if body is BAD:
                return out(400, {"error": "bad json"})
            acc = find(int(m.group(1)))
            if acc is None:
                return out(404, {"error": "no such account"})
            amount = body.get('amount') if isinstance(body, dict) else None
            if not (is_int(amount) and amount > 0):
                return out(400, {"error": "bad amount"})
            if m.group(2) == 'deposit':
                acc['balance'] += amount
            else:
                if amount > acc['balance']:
                    return out(409, {"error": "insufficient funds"})
                acc['balance'] -= amount
            return out(200, acc)

    return out(404, {"error": "not found"})


def main():
    for raw in sys.stdin.read().splitlines():
        if not raw.strip():
            continue
        parts = raw.split(' ', 2)
        method = parts[0]
        path = parts[1] if len(parts) > 1 else ''
        body_text = parts[2] if len(parts) > 2 else None
        print(handle(method, path, body_text))


main()
