import json
import sys
from collections import defaultdict
from decimal import Decimal, ROUND_HALF_EVEN

CENT = Decimal('0.01')
ZERO = Decimal('0')
TIER_RATE = {'gold': Decimal('0.10'), 'silver': Decimal('0.05'), 'basic': ZERO}
TAX_RATE = {'UK': Decimal('0.20'), 'US': Decimal('0.0825')}
CAT_RATE = Decimal('0.05')


def cents(x):
    return x.quantize(CENT, rounding=ROUND_HALF_EVEN)


def main():
    data = json.load(sys.stdin)
    products = data.get('products', {})
    customers = data.get('customers', {})
    orders = data.get('orders', [])
    invoices = []
    revenue = ZERO
    for order in orders:
        oid = order['id']
        cid = str(order['customer'])
        if cid not in customers:
            invoices.append({'id': oid, 'error': f'unknown customer {cid}'})
            continue
        cust = customers[cid]
        lines = []
        bad = None
        for ln in order['lines']:
            sku = ln['sku']
            if sku not in products:
                bad = sku
                break
            prod = products[sku]
            qty = int(ln['qty'])
            lines.append((prod['category'], qty, Decimal(prod['price']) * qty))
        if bad is not None:
            invoices.append({'id': oid, 'error': f'unknown sku {bad}'})
            continue

        catqty = defaultdict(int)
        for cat, qty, _ in lines:
            catqty[cat] += qty
        subtotal = sum((amt for _, _, amt in lines), ZERO)
        cat_disc = ZERO
        for cat, _, amt in lines:
            if catqty[cat] >= 3:
                cat_disc += cents(amt * CAT_RATE)
        net = subtotal - cat_disc
        tier_disc = cents(net * TIER_RATE.get(cust['tier'], ZERO))
        discount = cat_disc + tier_disc
        discounted = subtotal - discount
        tax = cents(discounted * TAX_RATE.get(cust['country'], ZERO))
        total = discounted + tax
        revenue += total
        invoices.append({
            'id': oid,
            'customer': cust['name'],
            'subtotal': str(cents(subtotal)),
            'discount': str(cents(discount)),
            'tax': str(cents(tax)),
            'total': str(cents(total)),
        })
    sys.stdout.write(
        '[' + ','.join(json.dumps(inv, separators=(',', ':')) for inv in invoices) + ']\n'
    )
    sys.stdout.write(f'revenue {cents(revenue)}\n')


main()
