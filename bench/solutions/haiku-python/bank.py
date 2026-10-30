import sys

def parse_amount(s):
    try:
        return int(s)
    except ValueError:
        return float(s)

def fmt(v):
    if isinstance(v, float) and v.is_integer():
        return str(int(v))
    return str(v)

balances = {}
for line in sys.stdin:
    parts = line.split()
    if not parts:
        continue
    cmd = parts[0]
    if cmd == "deposit":
        name, amt = parts[1], parse_amount(parts[2])
        balances[name] = balances.get(name, 0) + amt
    elif cmd == "withdraw":
        name, amt = parts[1], parse_amount(parts[2])
        bal = balances.get(name, 0)
        if amt > bal:
            print(f"error: insufficient funds for {name}")
        else:
            balances[name] = bal - amt
    elif cmd == "balance":
        name = parts[1]
        print(f"{name} {fmt(balances.get(name, 0))}")
