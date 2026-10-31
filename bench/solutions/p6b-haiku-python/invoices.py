import sys
import json
from decimal import Decimal, ROUND_HALF_EVEN

CENT = Decimal('0.01')
TIER_RATE = {'gold': Decimal('0.10'), 'silver': Decimal('0.05')}
TAX_RATE = {'UK': Decimal('0.20'), 'US': Decimal('0.0825')}


def q(x):
    return x.quantize(CENT, rounding=ROUND_HALF_EVEN)


def money(x):
    return f"{q(x):.2f}"


def main():
    data = json.loads(sys.stdin.read())
    products = data['products']
    customers = data['customers']
    orders = data['orders']

    results = []
    revenue = Decimal('0')

    for o in orders:
        oid = o['id']
        cust_key = str(o['customer'])
        cust = customers.get(cust_key)
        if cust is None:
            results.append({"id": oid, "error": "unknown customer " + cust_key})
            continue

        lines = []
        bad = None
        for ln in o['lines']:
            p = products.get(ln['sku'])
            if p is None:
                bad = ln['sku']
                break
            amount = q(Decimal(p['price']) * ln['qty'])
            lines.append((p['category'], amount, ln['qty']))
        if bad is not None:
            results.append({"id": oid, "error": "unknown sku " + bad})
            continue

        units = {}
        for cat, _, qty in lines:
            units[cat] = units.get(cat, 0) + qty

        subtotal = sum((amt for _, amt, _ in lines), Decimal('0'))
        discount = Decimal('0')
        for cat, amt, _ in lines:
            if units[cat] >= 3:
                discount += q(amt * Decimal('0.05'))

        after_cat = subtotal - discount
        tier = TIER_RATE.get(cust['tier'], Decimal('0'))
        discount += q(after_cat * tier)

        discounted = subtotal - discount
        tax = q(discounted * TAX_RATE.get(cust['country'], Decimal('0')))
        total = discounted + tax
        revenue += total

        results.append({
            "id": oid,
            "customer": cust['name'],
            "subtotal": money(subtotal),
            "discount": money(discount),
            "tax": money(tax),
            "total": money(total),
        })

    print(json.dumps(results, separators=(',', ':'), ensure_ascii=False))
    print(f"revenue {money(revenue)}")


main()
