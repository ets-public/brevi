import sys, json
from decimal import Decimal, ROUND_HALF_EVEN

C = Decimal('0.01')
def r(x):
    return x.quantize(C, rounding=ROUND_HALF_EVEN)
def s(x):
    return str(r(x))

data = json.load(sys.stdin)
prods = data.get('products', {})
custs = data.get('customers', {})
res = []
rev = Decimal('0')
TIER = {'gold': Decimal('0.10'), 'silver': Decimal('0.05')}
TAX = {'UK': Decimal('0.20'), 'US': Decimal('0.0825')}
for o in data.get('orders', []):
    oid = o['id']
    cid = o['customer']
    if cid not in custs:
        res.append({"id": oid, "error": "unknown customer " + str(cid)})
        continue
    bad = None
    for ln in o['lines']:
        if ln['sku'] not in prods:
            bad = ln['sku']; break
    if bad is not None:
        res.append({"id": oid, "error": "unknown sku " + bad})
        continue
    cu = custs[cid]
    catsub = {}
    catqty = {}
    sub = Decimal('0')
    for ln in o['lines']:
        p = prods[ln['sku']]
        amt = Decimal(p['price']) * ln['qty']
        sub += amt
        catsub[p['category']] = catsub.get(p['category'], Decimal('0')) + amt
        catqty[p['category']] = catqty.get(p['category'], 0) + ln['qty']
    disc = Decimal('0')
    for c, q in catqty.items():
        if q >= 3:
            disc += r(catsub[c] * Decimal('0.05'))
    disc += r((sub - disc) * TIER.get(cu['tier'], Decimal('0')))
    after = sub - disc
    tax = r(after * TAX.get(cu['country'], Decimal('0')))
    total = after + tax
    rev += r(total)
    res.append({"id": oid, "customer": cu['name'], "subtotal": s(sub), "discount": s(disc), "tax": s(tax), "total": s(total)})
print(json.dumps(res, separators=(',', ':')))
print('revenue ' + s(rev))
