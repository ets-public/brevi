import json
import re
import sys

accounts = []


def reply(code, obj):
    sys.stdout.write(f"{code} {json.dumps(obj, separators=(',', ':'))}\n")


def is_int(v):
    return isinstance(v, int) and not isinstance(v, bool)


def find_num(n):
    if is_int(n) and 1 <= n <= len(accounts):
        return accounts[n - 1]
    return None


def account_obj(a):
    return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}


def handle(line):
    parts = line.split(' ', 2)
    method = parts[0]
    path = parts[1] if len(parts) > 1 else ''
    body = parts[2] if len(parts) > 2 else None

    def get_body():
        try:
            data = json.loads(body)
        except (TypeError, ValueError):
            return None
        return data if isinstance(data, dict) else {}

    if path == '/accounts' and method == 'GET':
        ordered = sorted(accounts, key=lambda a: (-a['balance'], a['id']))
        reply(200, [account_obj(a) for a in ordered])
        return

    if path == '/accounts' and method == 'POST':
        data = get_body()
        if data is None:
            reply(400, {"error": "bad json"})
            return
        owner = data.get('owner')
        if not isinstance(owner, str) or owner == '':
            reply(400, {"error": "owner required"})
            return
        acc = {"id": len(accounts) + 1, "owner": owner, "balance": 0}
        accounts.append(acc)
        reply(201, account_obj(acc))
        return

    m = re.fullmatch(r'/accounts/([0-9]+)', path)
    if m and method == 'GET':
        acc = find_num(int(m.group(1)))
        if acc is None:
            reply(404, {"error": "no such account"})
        else:
            reply(200, account_obj(acc))
        return

    m = re.fullmatch(r'/accounts/([0-9]+)/(deposit|withdraw)', path)
    if m and method == 'POST':
        data = get_body()
        if data is None:
            reply(400, {"error": "bad json"})
            return
        acc = find_num(int(m.group(1)))
        if acc is None:
            reply(404, {"error": "no such account"})
            return
        amt = data.get('amount')
        if not is_int(amt) or amt <= 0:
            reply(400, {"error": "bad amount"})
            return
        if m.group(2) == 'deposit':
            acc['balance'] += amt
        else:
            if amt > acc['balance']:
                reply(409, {"error": "insufficient funds"})
                return
            acc['balance'] -= amt
        reply(200, account_obj(acc))
        return

    if path == '/transfer' and method == 'POST':
        data = get_body()
        if data is None:
            reply(400, {"error": "bad json"})
            return
        src = find_num(data.get('from'))
        dst = find_num(data.get('to'))
        if src is None or dst is None:
            reply(404, {"error": "no such account"})
            return
        if src is dst:
            reply(400, {"error": "same account"})
            return
        amt = data.get('amount')
        if not is_int(amt) or amt <= 0:
            reply(400, {"error": "bad amount"})
            return
        if amt > src['balance']:
            reply(409, {"error": "insufficient funds"})
            return
        src['balance'] -= amt
        dst['balance'] += amt
        reply(200, {"from": src['balance'], "to": dst['balance']})
        return

    reply(404, {"error": "not found"})


def main():
    for raw in sys.stdin.read().split('\n'):
        line = raw.rstrip('\r')
        if line.strip() == '':
            continue
        handle(line)


main()
