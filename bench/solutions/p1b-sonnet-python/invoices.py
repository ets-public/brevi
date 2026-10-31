import sys, json
from decimal import Decimal, ROUND_HALF_EVEN

data = json.loads(sys.stdin.read())
products = data.get("products", {})
customers = data.get("customers", {})
C = Decimal("0.01")


def rnd(x):
    return x.quantize(C, rounding=ROUND_HALF_EVEN)


def fmt(x):
    return "%s" % x.quantize(C)


res = []
revenue = Decimal("0")
for o in data.get("orders", []):
    oid = o["id"]
    cid = o["customer"]
    if cid not in customers:
        res.append({"id": oid, "error": "unknown customer %s" % cid})
        continue
    bad = None
    for ln in o["lines"]:
        if ln["sku"] not in products:
            bad = ln["sku"]
            break
    if bad is not None:
        res.append({"id": oid, "error": "unknown sku %s" % bad})
        continue
    cust = customers[cid]
    cat_amt = {}
    cat_qty = {}
    subtotal = Decimal("0")
    for ln in o["lines"]:
        p = products[ln["sku"]]
        amt = Decimal(p["price"]) * ln["qty"]
        subtotal += amt
        c = p["category"]
        cat_amt[c] = cat_amt.get(c, Decimal("0")) + amt
        cat_qty[c] = cat_qty.get(c, 0) + ln["qty"]
    disc = Decimal("0")
    for c in cat_amt:
        if cat_qty[c] >= 3:
            disc += rnd(cat_amt[c] * Decimal("0.05"))
    rate = {"gold": Decimal("0.10"), "silver": Decimal("0.05")}.get(cust["tier"], Decimal("0"))
    disc += rnd((subtotal - disc) * rate)
    net = subtotal - disc
    trate = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}.get(cust["country"], Decimal("0"))
    tax = rnd(net * trate)
    total = net + tax
    revenue += total
    res.append({"id": oid, "customer": cust["name"], "subtotal": fmt(subtotal), "discount": fmt(disc), "tax": fmt(tax), "total": fmt(total)})
print(json.dumps(res, separators=(",", ":"), ensure_ascii=False))
print("revenue " + fmt(revenue))
