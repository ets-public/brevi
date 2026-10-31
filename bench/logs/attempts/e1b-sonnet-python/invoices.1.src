import sys, json
from decimal import Decimal, ROUND_HALF_EVEN
Q = Decimal('0.01')
def rnd(x):
    return x.quantize(Q, rounding=ROUND_HALF_EVEN)
d = json.loads(sys.stdin.read())
prods = d["products"]; custs = d["customers"]
res = []; rev = Decimal('0.00')
for o in d["orders"]:
    oid = o["id"]
    c = custs.get(o["customer"])
    if c is None:
        res.append({"id": oid, "error": "unknown customer %s" % o["customer"]}); continue
    bad = None
    for ln in o["lines"]:
        if ln["sku"] not in prods:
            bad = ln["sku"]; break
    if bad is not None:
        res.append({"id": oid, "error": "unknown sku %s" % bad}); continue
    sub = Decimal('0'); catamt = {}; catqty = {}
    for ln in o["lines"]:
        p = prods[ln["sku"]]
        a = Decimal(p["price"]) * ln["qty"]
        sub += a
        cat = p["category"]
        catamt[cat] = catamt.get(cat, Decimal('0')) + a
        catqty[cat] = catqty.get(cat, 0) + ln["qty"]
    disc = Decimal('0')
    for cat in catamt:
        if catqty[cat] >= 3:
            disc += rnd(catamt[cat] * Decimal('0.05'))
    rate = {"gold": Decimal('0.10'), "silver": Decimal('0.05')}.get(c["tier"], Decimal('0'))
    disc += rnd((sub - disc) * rate)
    after = sub - disc
    tr = {"UK": Decimal('0.20'), "US": Decimal('0.0825')}.get(c["country"], Decimal('0'))
    tax = rnd(after * tr)
    total = after + tax
    rev += total
    f = lambda x: str(rnd(x))
    res.append({"id": oid, "customer": c["name"], "subtotal": f(sub), "discount": f(disc), "tax": f(tax), "total": f(total)})
print(json.dumps(res, separators=(',', ':')))
print("revenue " + str(rnd(rev)))
