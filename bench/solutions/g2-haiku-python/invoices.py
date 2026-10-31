import json
import sys
from decimal import Decimal, ROUND_HALF_EVEN

CENT = Decimal("0.01")


def money(d):
    return d.quantize(CENT, rounding=ROUND_HALF_EVEN)


def fmt(d):
    return "%.2f" % money(d)


def main():
    data = json.loads(sys.stdin.read())
    products = data.get("products", {})
    customers = data.get("customers", {})
    orders = data.get("orders", [])

    invoices = []
    revenue = Decimal("0")

    for order in orders:
        oid = order.get("id")
        cust_id = str(order.get("customer"))
        cust = customers.get(cust_id)
        if cust is None:
            invoices.append({"id": oid, "error": "unknown customer %s" % cust_id})
            continue

        lines = order.get("lines", [])
        bad_sku = None
        for ln in lines:
            if ln["sku"] not in products:
                bad_sku = ln["sku"]
                break
        if bad_sku is not None:
            invoices.append({"id": oid, "error": "unknown sku %s" % bad_sku})
            continue

        # Per-line amounts, and units per category.
        priced = []
        units = {}
        for ln in lines:
            prod = products[ln["sku"]]
            qty = int(ln["qty"])
            amount = Decimal(str(prod["price"])) * qty
            cat = prod["category"]
            units[cat] = units.get(cat, 0) + qty
            priced.append((amount, cat))

        subtotal = sum((a for a, _ in priced), Decimal("0"))

        # Category discount: 5% on lines of categories with >= 3 units, each rounded.
        discount = Decimal("0")
        for amount, cat in priced:
            if units[cat] >= 3:
                discount += money(amount * Decimal("0.05"))

        # Tier discount on the amount after category discounts.
        rate = {"gold": Decimal("0.10"), "silver": Decimal("0.05")}.get(cust["tier"], Decimal("0"))
        after_cat = subtotal - discount
        discount += money(after_cat * rate)

        discounted = subtotal - discount
        country = cust["country"]
        tax_rate = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}.get(country, Decimal("0"))
        tax = money(discounted * tax_rate)
        total = discounted + tax
        revenue += total

        invoices.append({
            "id": oid,
            "customer": cust["name"],
            "subtotal": fmt(subtotal),
            "discount": fmt(discount),
            "tax": fmt(tax),
            "total": fmt(total),
        })

    sys.stdout.write(json.dumps(invoices, separators=(",", ":"), ensure_ascii=False) + "\n")
    sys.stdout.write("revenue %s\n" % fmt(revenue))


if __name__ == "__main__":
    main()
