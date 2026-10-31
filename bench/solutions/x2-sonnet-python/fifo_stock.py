import sys, re, datetime

def money(c):
    s = "-" if c < 0 else ""
    c = abs(c)
    return "%s%d.%02d" % (s, c // 100, c % 100)

def price(s):
    if not re.fullmatch(r"\d+(\.\d{1,2})?", s):
        return None
    if '.' in s:
        a, b = s.split('.')
        return int(a) * 100 + int(b.ljust(2, '0'))
    return int(s) * 100

def main():
    skus = {}
    months = {}
    last = None
    out = []
    lines = sys.stdin.read().split('\n')
    for n, line in enumerate(lines, 1):
        t = line.split()
        if not t:
            continue
        ok = False
        if len(t) >= 4 and re.fullmatch(r"\d{4}-\d{2}-\d{2}", t[0]) and re.fullmatch(r"\d+", t[3]) and int(t[3]) >= 1:
            try:
                datetime.date(int(t[0][:4]), int(t[0][5:7]), int(t[0][8:]))
                dok = True
            except ValueError:
                dok = False
            op = t[1]
            if dok:
                if op in ("RECEIVE", "SHIP") and len(t) == 5 and price(t[4]) is not None:
                    ok = True
                elif op == "LOSS" and len(t) == 4:
                    ok = True
        if not ok:
            out.append("line %d: bad line" % n)
            continue
        date, op, sku, qty = t[0], t[1], t[2], int(t[3])
        if last is not None and date < last:
            out.append("line %d: date out of order" % n)
            continue
        p = price(t[4]) if len(t) == 5 else 0
        s = skus.get(sku)
        if op == "RECEIVE":
            last = date
            if s is None:
                s = skus[sku] = {"lots": [], "cogs": 0, "rev": 0, "loss": 0}
            s["lots"].append([qty, p])
            continue
        have = sum(l[0] for l in s["lots"]) if s else 0
        if qty > have:
            out.append("line %d: short %s need %d have %d" % (n, sku, qty, have))
            continue
        last = date
        cost = 0
        need = qty
        lots = s["lots"]
        while need > 0:
            l = lots[0]
            q = min(need, l[0])
            cost += q * l[1]
            l[0] -= q
            need -= q
            if l[0] == 0:
                lots.pop(0)
        m = months.setdefault(date[:7], [0, 0])
        if op == "SHIP":
            s["cogs"] += cost
            s["rev"] += qty * p
            m[0] += qty * p - cost
        else:
            s["loss"] += cost
            m[1] += cost
    tp = tl = 0
    for sku in sorted(skus):
        s = skus[sku]
        onh = sum(l[0] for l in s["lots"])
        val = sum(l[0] * l[1] for l in s["lots"])
        profit = s["rev"] - s["cogs"]
        tp += profit
        tl += s["loss"]
        out.append("%s on_hand %d value %s cogs %s revenue %s profit %s loss %s" % (
            sku, onh, money(val), money(s["cogs"]), money(s["rev"]), money(profit), money(s["loss"])))
    out.append("months")
    for k in sorted(months):
        out.append("  %s profit %s loss %s" % (k, money(months[k][0]), money(months[k][1])))
    out.append("total profit %s loss %s net %s" % (money(tp), money(tl), money(tp - tl)))
    print("\n".join(out))

main()
