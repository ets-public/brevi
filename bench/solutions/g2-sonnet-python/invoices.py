import sys, json
from decimal import Decimal, ROUND_HALF_EVEN
C = Decimal("0.01")
def r(x): return x.quantize(C, rounding=ROUND_HALF_EVEN)
def m(x): return "%s" % r(x)
data = json.loads(sys.stdin.read())
prods = data.get("products", {}); custs = data.get("customers", {})
res = []; revenue = Decimal(0)
tier = {"gold": Decimal("0.10"), "silver": Decimal("0.05")}
tax = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}
for o in data.get("orders", []):
    oid = o["id"]; cid = o["customer"]
    if str(cid) not in custs:
        res.append({"id": oid, "error": "unknown customer %s" % cid}); continue
    bad = None
    for ln in o["lines"]:
        if ln["sku"] not in prods:
            bad = ln["sku"]; break
    if bad is not None:
        res.append({"id": oid, "error": "unknown sku %s" % bad}); continue
    cu = custs[str(cid)]
    sub = Decimal(0); catamt = {}; catqty = {}
    for ln in o["lines"]:
        p = prods[ln["sku"]]
        a = Decimal(p["price"]) * ln["qty"]
        sub += a
        catamt[p["category"]] = catamt.get(p["category"], 0) + a
        catqty[p["category"]] = catqty.get(p["category"], 0) + ln["qty"]
    disc = Decimal(0)
    for c, q in catqty.items():
        if q >= 3:
            disc += r(catamt[c] * Decimal("0.05"))
    disc += r((sub - disc) * tier.get(cu["tier"], Decimal(0)))
    t = r((sub - disc) * tax.get(cu["country"], Decimal(0)))
    total = sub - disc + t
    revenue += total
    res.append({"id": oid, "customer": cu["name"], "subtotal": m(sub), "discount": m(disc), "tax": m(t), "total": m(total)})
print(json.dumps(res, separators=(",", ":")))
print("revenue " + m(revenue))
