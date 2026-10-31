import sys
import json
from decimal import Decimal, ROUND_HALF_EVEN

CENT = Decimal('0.01')
TIER = {'gold': Decimal('0.10'), 'silver': Decimal('0.05'), 'basic': Decimal('0')}
TAXRATE = {'UK': Decimal('0.20'), 'US': Decimal('0.0825')}


def q(x):
    return x.quantize(CENT, rounding=ROUND_HALF_EVEN)


def money(x):
    return format(q(x), 'f')


def main():
    d = json.loads(sys.stdin.read())
    products = d.get('products', {})
    customers = d.get('customers', {})
    orders = d.get('orders', [])

    results = []
    revenue = Decimal('0')
    for o in orders:
        oid = o['id']
        cid = o['customer']
        cust = customers.get(str(cid))
        if cust is None:
            results.append({"id": oid, "error": f"unknown customer {cid}"})
            continue
        bad = None
        for l in o['lines']:
            if l['sku'] not in products:
                bad = l['sku']
                break
        if bad is not None:
            results.append({"id": oid, "error": f"unknown sku {bad}"})
            continue

        lines = []
        for l in o['lines']:
            p = products[l['sku']]
            qty = int(l['qty'])
            amt = Decimal(p['price']) * qty
            lines.append((p['category'], qty, amt))

        catq = {}
        for cat, qty, _ in lines:
            catq[cat] = catq.get(cat, 0) + qty

        subtotal = sum((amt for _, _, amt in lines), Decimal('0'))
        cat_disc = Decimal('0')
        for cat, _, amt in lines:
            if catq[cat] >= 3:
                cat_disc += q(amt * Decimal('0.05'))
        after_cat = subtotal - cat_disc
        tier_disc = q(after_cat * TIER.get(cust['tier'], Decimal('0')))
        discount = cat_disc + tier_disc
        taxable = subtotal - discount
        tax = q(taxable * TAXRATE.get(cust['country'], Decimal('0')))
        total = subtotal - discount + tax
        revenue += total
        results.append({
            "id": oid,
            "customer": cust['name'],
            "subtotal": money(subtotal),
            "discount": money(discount),
            "tax": money(tax),
            "total": money(total),
        })

    print(json.dumps(results, separators=(',', ':')))
    print(f"revenue {money(revenue)}")


if __name__ == '__main__':
    main()
