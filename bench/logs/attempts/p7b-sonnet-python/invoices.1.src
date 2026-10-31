import sys, json
from decimal import Decimal, ROUND_HALF_EVEN
D = json.loads(sys.stdin.read(), parse_float=Decimal)
C = Decimal('0.01')
def r(x):
    return x.quantize(C, rounding=ROUND_HALF_EVEN)
def f(x):
    return '%.2f' % r(x)
tier = {'gold': Decimal('0.10'), 'silver': Decimal('0.05')}
out = []
rev = Decimal('0.00')
for o in D['orders']:
    cu = D['customers'].get(o['customer'])
    if cu is None:
        out.append({"id": o['id'], "error": "unknown customer %s" % o['customer']})
        continue
    bad = None
    for l in o['lines']:
        if l['sku'] not in D['products']:
            bad = l['sku']; break
    if bad is not None:
        out.append({"id": o['id'], "error": "unknown sku %s" % bad})
        continue
    sub = Decimal('0'); cat = {}; qty = {}
    for l in o['lines']:
        p = D['products'][l['sku']]
        amt = Decimal(p['price']) * l['qty']
        sub += amt
        cat[p['category']] = cat.get(p['category'], Decimal('0')) + amt
        qty[p['category']] = qty.get(p['category'], 0) + l['qty']
    disc = Decimal('0')
    after = sub
    for c in cat:
        if qty[c] >= 3:
            d = r(cat[c] * Decimal('0.05'))
            disc += d; after -= d
    td = r(after * tier.get(cu['tier'], Decimal('0')))
    disc += td; after -= td
    rate = {'UK': Decimal('0.20'), 'US': Decimal('0.0825')}.get(cu['country'], Decimal('0'))
    tax = r(after * rate)
    total = r(sub) - disc + tax
    rev += total
    out.append({"id": o['id'], "customer": cu['name'], "subtotal": f(sub), "discount": f(disc), "tax": f(tax), "total": f(total)})
print(json.dumps(out, separators=(',', ':')))
print('revenue ' + f(rev))
