import json
import re
import sys

accounts = {}  # id -> {"id", "owner", "balance"}
next_id = 1

ID_RE = r'(\d+)'
ROUTES = [
    ('POST', re.compile(r'^/accounts$'), 'create'),
    ('GET', re.compile(r'^/accounts$'), 'list'),
    ('GET', re.compile(r'^/accounts/' + ID_RE + r'$'), 'get'),
    ('POST', re.compile(r'^/accounts/' + ID_RE + r'/deposit$'), 'deposit'),
    ('POST', re.compile(r'^/accounts/' + ID_RE + r'/withdraw$'), 'withdraw'),
    ('POST', re.compile(r'^/transfer$'), 'transfer'),
]


def dump(obj):
    return json.dumps(obj, separators=(',', ':'))


def resp(status, obj):
    return '%d %s' % (status, dump(obj))


NOT_FOUND = (404, {"error": "not found"})
NO_ACCOUNT = (404, {"error": "no such account"})
BAD_AMOUNT = (400, {"error": "bad amount"})
BAD_JSON = (400, {"error": "bad json"})


def valid_amount(v):
    return type(v) is int and v > 0


def account_obj(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}


def handle(method, path, body_text):
    global next_id
    route = None
    for m, rx, name in ROUTES:
        if m != method:
            continue
        mt = rx.match(path)
        if mt:
            route = (name, mt.groups())
            break
    if route is None:
        return resp(*NOT_FOUND)
    name, groups = route

    body = {}
    if body_text is not None:
        try:
            body = json.loads(body_text)
        except ValueError:
            return resp(*BAD_JSON)
        if not isinstance(body, dict):
            body = {}

    if name == 'create':
        owner = body.get('owner')
        if not isinstance(owner, str) or owner == '':
            return resp(400, {"error": "owner required"})
        aid = next_id
        next_id += 1
        accounts[aid] = {"id": aid, "owner": owner, "balance": 0}
        return resp(201, account_obj(accounts[aid]))

    if name == 'list':
        items = sorted(accounts.values(), key=lambda a: (-a["balance"], a["id"]))
        return resp(200, [account_obj(a) for a in items])

    if name in ('get', 'deposit', 'withdraw'):
        aid = int(groups[0])
        if aid not in accounts:
            return resp(*NO_ACCOUNT)
        acc = accounts[aid]
        if name == 'get':
            return resp(200, account_obj(acc))
        amount = body.get('amount')
        if not valid_amount(amount):
            return resp(*BAD_AMOUNT)
        if name == 'deposit':
            acc["balance"] += amount
            return resp(200, account_obj(acc))
        if amount > acc["balance"]:
            return resp(409, {"error": "insufficient funds"})
        acc["balance"] -= amount
        return resp(200, account_obj(acc))

    if name == 'transfer':
        src = body.get('from')
        dst = body.get('to')
        if type(src) is not int or src not in accounts:
            return resp(*NO_ACCOUNT)
        if type(dst) is not int or dst not in accounts:
            return resp(*NO_ACCOUNT)
        if src == dst:
            return resp(400, {"error": "same account"})
        amount = body.get('amount')
        if not valid_amount(amount):
            return resp(*BAD_AMOUNT)
        a = accounts[src]
        b = accounts[dst]
        if amount > a["balance"]:
            return resp(409, {"error": "insufficient funds"})
        a["balance"] -= amount
        b["balance"] += amount
        return resp(200, {"from": a["balance"], "to": b["balance"]})

    return resp(*NOT_FOUND)


def main():
    out = []
    for raw in sys.stdin:
        line = raw.rstrip('\r\n')
        if not line.strip():
            continue
        parts = line.split(' ', 2)
        method = parts[0]
        path = parts[1] if len(parts) > 1 else ''
        body_text = parts[2] if len(parts) > 2 else None
        out.append(handle(method, path, body_text))
    if out:
        sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
