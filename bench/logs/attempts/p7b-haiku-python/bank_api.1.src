import sys
import json
import re

accounts = []  # account i (id i+1) is accounts[i]


def out(code, obj):
    print(f"{code} {json.dumps(obj, separators=(',', ':'))}")


def err(code, msg):
    out(code, {"error": msg})


def acct_obj(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}


def get_acct(aid):
    if aid is None:
        return None
    if 1 <= aid <= len(accounts):
        return accounts[aid - 1]
    return None


def id_of(v):
    return v if type(v) is int else None


def valid_amount(v):
    return type(v) is int and v > 0


def handle(line):
    parts = line.split(' ', 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ''
    body = parts[2] if len(parts) > 2 else None

    route = None
    if path == '/accounts':
        if method == 'POST':
            route = ('create', None)
        elif method == 'GET':
            route = ('list', None)
    elif path == '/transfer' and method == 'POST':
        route = ('transfer', None)
    else:
        m = re.fullmatch(r'/accounts/([0-9]+)(/(deposit|withdraw))?', path)
        if m:
            aid = int(m.group(1))
            if m.group(3) is None and method == 'GET':
                route = ('get', aid)
            elif m.group(3) is not None and method == 'POST':
                route = (m.group(3), aid)
    if route is None:
        err(404, 'not found')
        return

    data = {}
    if body is not None and body.strip() != '':
        try:
            data = json.loads(body)
        except ValueError:
            err(400, 'bad json')
            return
        if not isinstance(data, dict):
            data = {}

    kind, aid = route

    if kind == 'list':
        res = sorted(accounts, key=lambda a: (-a['balance'], a['id']))
        out(200, [acct_obj(a) for a in res])
        return

    if kind == 'create':
        owner = data.get('owner')
        if not isinstance(owner, str) or owner == '':
            err(400, 'owner required')
            return
        a = {'id': len(accounts) + 1, 'owner': owner, 'balance': 0}
        accounts.append(a)
        out(201, acct_obj(a))
        return

    if kind == 'get':
        a = get_acct(aid)
        if a is None:
            err(404, 'no such account')
            return
        out(200, acct_obj(a))
        return

    if kind in ('deposit', 'withdraw'):
        a = get_acct(aid)
        if a is None:
            err(404, 'no such account')
            return
        amt = data.get('amount')
        if not valid_amount(amt):
            err(400, 'bad amount')
            return
        if kind == 'withdraw':
            if amt > a['balance']:
                err(409, 'insufficient funds')
                return
            a['balance'] -= amt
        else:
            a['balance'] += amt
        out(200, acct_obj(a))
        return

    if kind == 'transfer':
        src = get_acct(id_of(data.get('from')))
        dst = get_acct(id_of(data.get('to')))
        if src is None or dst is None:
            err(404, 'no such account')
            return
        if src is dst:
            err(400, 'same account')
            return
        amt = data.get('amount')
        if not valid_amount(amt):
            err(400, 'bad amount')
            return
        if amt > src['balance']:
            err(409, 'insufficient funds')
            return
        src['balance'] -= amt
        dst['balance'] += amt
        out(200, {'from': src['balance'], 'to': dst['balance']})
        return


def main():
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        handle(line)


if __name__ == '__main__':
    main()
