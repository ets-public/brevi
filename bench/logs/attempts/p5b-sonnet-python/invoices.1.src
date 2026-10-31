import sys, json
from decimal import Decimal, ROUND_HALF_EVEN
C = Decimal("0.01")
def r(x):
    return x.quantize(C, rounding=ROUND_HALF_EVEN)
d = json.loads(sys.stdin.read(), parse_float=Decimal)
prods = d.get("products", {}); custs = d.get("customers", {})
out = []; rev = Decimal(0)
for o in d.get("orders", []):
    oid = o["id"]; cid = o["customer"]
    if cid not in custs:
        out.append({"id": oid, "error": "unknown customer %s" % cid}); continue
    bad = None
    for l in o["lines"]:
        if l["sku"] not in prods:
            bad = l["sku"]; break
    if bad is not None:
        out.append({"id": oid, "error": "unknown sku %s" % bad}); continue
    cu = custs[cid]
    cat_amt = {}; cat_q = {}; sub = Decimal(0)
    for l in o["lines"]:
        p = prods[l["sku"]]
        a = Decimal(p["price"]) * l["qty"]
        sub += a
        cat_amt[p["category"]] = cat_amt.get(p["category"], Decimal(0)) + a
        cat_q[p["category"]] = cat_q.get(p["category"], 0) + l["qty"]
    disc = Decimal(0)
    for c, q in cat_q.items():
        if q >= 3:
            disc += r(cat_amt[c] * Decimal("0.05"))
    rate = {"gold": Decimal("0.10"), "silver": Decimal("0.05")}.get(cu["tier"], Decimal(0))
    disc += r((sub - disc) * rate)
    after = sub - disc
    tr = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}.get(cu["country"], Decimal(0))
    tax = r(after * tr)
    total = after + tax
    rev += total
    out.append({"id": oid, "customer": cu["name"], "subtotal": str(r(sub)), "discount": str(r(disc)), "tax": str(tax), "total": str(r(total))})
print(json.dumps(out, separators=(',', ':'), ensure_ascii=False))
print("revenue %s" % r(rev))
