import sys, json
from decimal import Decimal, ROUND_HALF_EVEN, getcontext

getcontext().prec = 50
C = Decimal("0.01")

def q(x):
    return x.quantize(C, rounding=ROUND_HALF_EVEN)

def fmt(x):
    return "{:.2f}".format(q(x))

d = json.loads(sys.stdin.read())
products = d.get("products", {})
customers = d.get("customers", {})
res = []
revenue = Decimal(0)
for o in d.get("orders", []):
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
    catamt = {}
    catqty = {}
    for l in o["lines"]:
        p = products[l["sku"]]
        amt = Decimal(p["price"]) * l["qty"]
        sub += amt
        c = p["category"]
        catamt[c] = catamt.get(c, Decimal(0)) + amt
        catqty[c] = catqty.get(c, 0) + l["qty"]
    disc = Decimal(0)
    for c in catamt:
        if catqty[c] >= 3:
            disc += q(catamt[c] * Decimal("0.05"))
    rate = {"gold": Decimal("0.10"), "silver": Decimal("0.05")}.get(cust["tier"], Decimal(0))
    disc += q((sub - disc) * rate)
    net = sub - disc
    tr = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}.get(cust["country"], Decimal(0))
    tax = q(net * tr)
    total = net + tax
    revenue += total
    res.append({"id": oid, "customer": cust["name"], "subtotal": fmt(sub), "discount": fmt(disc), "tax": fmt(tax), "total": fmt(total)})
print(json.dumps(res, separators=(",", ":")))
print("revenue " + fmt(revenue))
