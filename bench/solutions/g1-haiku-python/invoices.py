import json
import sys
from decimal import Decimal, ROUND_HALF_EVEN

CENT = Decimal('0.01')
TIER_RATE = {'gold': Decimal('0.10'), 'silver': Decimal('0.05'), 'basic': Decimal('0')}
TAX_RATE = {'UK': Decimal('0.20'), 'US': Decimal('0.0825')}
CAT_RATE = Decimal('0.05')


def rnd(d):
    return d.quantize(CENT, rounding=ROUND_HALF_EVEN)


def money(d):
    return '{:.2f}'.format(rnd(d))


def main():
    data = json.loads(sys.stdin.read())
    products = data.get('products', {})
    customers = data.get('customers', {})
    orders = data.get('orders', [])

    out = []
    revenue = Decimal('0')

    for order in orders:
        oid = order.get('id')
        cid = order.get('customer')
        lines = order.get('lines', [])

        cust = customers.get(str(cid))
        if cust is None:
            out.append({"id": oid, "error": "unknown customer %s" % cid})
            continue

        # Resolve SKUs, first unknown wins.
        bad_sku = None
        priced = []
        for ln in lines:
            sku = ln.get('sku')
            if sku not in products:
                bad_sku = sku
                break
            qty = int(ln.get('qty', 0))
            price = Decimal(products[sku]['price'])
            cat = products[sku].get('category')
            priced.append((cat, qty, price * qty))
        if bad_sku is not None:
            out.append({"id": oid, "error": "unknown sku %s" % bad_sku})
            continue

        subtotal = sum((amt for _, _, amt in priced), Decimal('0'))

        units = {}
        for cat, qty, _ in priced:
            units[cat] = units.get(cat, 0) + qty

        discount = Decimal('0')
        after_cat = Decimal('0')
        for cat, qty, amt in priced:
            line_disc = Decimal('0')
            if units[cat] >= 3:
                line_disc = rnd(amt * CAT_RATE)
            discount += line_disc
            after_cat += amt - line_disc

        tier_disc = rnd(after_cat * TIER_RATE.get(cust.get('tier'), Decimal('0')))
        discount += tier_disc

        discounted = subtotal - discount
        tax = rnd(discounted * TAX_RATE.get(cust.get('country'), Decimal('0')))
        total = discounted + tax
        revenue += total

        out.append({
            "id": oid,
            "customer": cust.get('name'),
            "subtotal": money(subtotal),
            "discount": money(discount),
            "tax": money(tax),
            "total": money(total),
        })

    sys.stdout.write(json.dumps(out, separators=(',', ':')) + '\n')
    sys.stdout.write('revenue ' + money(revenue) + '\n')


if __name__ == '__main__':
    main()
