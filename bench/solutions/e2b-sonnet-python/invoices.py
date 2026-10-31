import sys, json
from decimal import Decimal, ROUND_HALF_EVEN

C = Decimal("0.01")


def q(x):
    return x.quantize(C, rounding=ROUND_HALF_EVEN)


data = json.loads(sys.stdin.read())
products = data.get("products", {})
customers = data.get("customers", {})
out = []
rev = Decimal("0.00")
for o in data.get("orders", []):
    oid = o["id"]
    cid = o["customer"]
    if cid not in customers:
        out.append({"id": oid, "error": "unknown customer %s" % cid})
        continue
    bad = None
    for l in o["lines"]:
        if l["sku"] not in products:
            bad = l["sku"]
            break
    if bad is not None:
        out.append({"id": oid, "error": "unknown sku %s" % bad})
        continue
    cust = customers[cid]
    sub = Decimal("0")
    catqty = {}
    catamt = {}
    for l in o["lines"]:
        p = products[l["sku"]]
        amt = Decimal(p["price"]) * l["qty"]
        sub += amt
        c = p["category"]
        catqty[c] = catqty.get(c, 0) + l["qty"]
        catamt[c] = catamt.get(c, Decimal("0")) + amt
    disc = Decimal("0")
    for c in catqty:
        if catqty[c] >= 3:
            disc += q(catamt[c] * Decimal("0.05"))
    rate = {"gold": Decimal("0.10"), "silver": Decimal("0.05")}.get(cust["tier"], Decimal("0"))
    disc += q((sub - disc) * rate)
    after = sub - disc
    trate = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}.get(cust["country"], Decimal("0"))
    tax = q(after * trate)
    total = after + tax
    rev += total
    out.append({"id": oid, "customer": cust["name"], "subtotal": str(q(sub)),
                "discount": str(q(disc)), "tax": str(q(tax)), "total": str(q(total))})
print(json.dumps(out, separators=(',', ':')))
print("revenue %s" % q(rev))
