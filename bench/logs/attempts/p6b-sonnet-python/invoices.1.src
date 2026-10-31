import sys, json
from decimal import Decimal, ROUND_HALF_EVEN
C = Decimal('0.01')
def q(x):
    return x.quantize(C, rounding=ROUND_HALF_EVEN)
d = json.loads(sys.stdin.read())
prods = d["products"]; custs = d["customers"]
res = []; rev = Decimal('0.00')
tier = {"gold": Decimal('0.10'), "silver": Decimal('0.05')}
tax = {"UK": Decimal('0.20'), "US": Decimal('0.0825')}
for o in d["orders"]:
    oid = o["id"]
    cid = o["customer"]
    if cid not in custs:
        res.append({"id": oid, "error": "unknown customer %s" % cid}); continue
    bad = None
    for l in o["lines"]:
        if l["sku"] not in prods:
            bad = l["sku"]; break
    if bad is not None:
        res.append({"id": oid, "error": "unknown sku %s" % bad}); continue
    cu = custs[cid]
    qty = {}; amt = {}
    sub = Decimal('0')
    for l in o["lines"]:
        p = prods[l["sku"]]
        a = Decimal(p["price"]) * l["qty"]
        c = p["category"]
        qty[c] = qty.get(c, 0) + l["qty"]
        amt[c] = amt.get(c, Decimal('0')) + a
        sub += a
    disc = Decimal('0')
    for c in qty:
        if qty[c] >= 3:
            disc += q(amt[c] * Decimal('0.05'))
    disc += q((sub - disc) * tier.get(cu["tier"], Decimal('0')))
    net = sub - disc
    t = q(net * tax.get(cu["country"], Decimal('0')))
    total = net + t
    rev += total
    res.append({"id": oid, "customer": cu["name"], "subtotal": str(q(sub)), "discount": str(q(disc)), "tax": str(t), "total": str(q(total))})
print(json.dumps(res, separators=(',', ':')))
print('revenue %s' % q(rev))
