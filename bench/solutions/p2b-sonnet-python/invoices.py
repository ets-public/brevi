import sys, json
from decimal import Decimal, ROUND_HALF_EVEN, getcontext
getcontext().prec = 50
C = Decimal("0.01")

def rnd(x):
    return x.quantize(C, rounding=ROUND_HALF_EVEN)

def fmt(x):
    return "%s" % x.quantize(C, rounding=ROUND_HALF_EVEN)

data = json.loads(sys.stdin.read())
products = data.get("products", {})
customers = data.get("customers", {})
res = []
revenue = Decimal(0)
for o in data.get("orders", []):
    oid = o["id"]
    cid = o["customer"]
    if cid not in customers:
        res.append({"id": oid, "error": "unknown customer %s" % cid})
        continue
    bad = None
    for l in o["lines"]:
        if l["sku"] not in products:
            bad = l["sku"]
            break
    if bad is not None:
        res.append({"id": oid, "error": "unknown sku %s" % bad})
        continue
    cust = customers[cid]
    sub = Decimal(0)
    cat_amt = {}
    cat_qty = {}
    for l in o["lines"]:
        p = products[l["sku"]]
        amt = Decimal(p["price"]) * l["qty"]
        sub += amt
        c = p["category"]
        cat_amt[c] = cat_amt.get(c, Decimal(0)) + amt
        cat_qty[c] = cat_qty.get(c, 0) + l["qty"]
    disc = Decimal(0)
    for c in cat_amt:
        if cat_qty[c] >= 3:
            disc += rnd(cat_amt[c] * Decimal("0.05"))
    after = sub - disc
    pct = {"gold": Decimal("0.10"), "silver": Decimal("0.05")}.get(cust["tier"], Decimal(0))
    disc += rnd(after * pct)
    base = sub - disc
    rate = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}.get(cust["country"], Decimal(0))
    tax = rnd(base * rate)
    total = base + tax
    revenue += rnd(total)
    res.append({"id": oid, "customer": cust["name"], "subtotal": fmt(sub), "discount": fmt(disc), "tax": fmt(tax), "total": fmt(total)})
print(json.dumps(res, separators=(',', ':')))
print("revenue " + fmt(revenue))
