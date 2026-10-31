import json
import sys
from decimal import Decimal, ROUND_HALF_EVEN
from collections import defaultdict

CENT = Decimal("0.01")
TIER_RATE = {"gold": Decimal("0.10"), "silver": Decimal("0.05"), "basic": Decimal("0")}
TAX_RATE = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}


def cents(x):
    return x.quantize(CENT, rounding=ROUND_HALF_EVEN)


def fmt(x):
    return f"{cents(x):.2f}"


def main():
    data = json.loads(sys.stdin.read())
    products = data.get("products", {})
    customers = data.get("customers", {})
    orders = data.get("orders", [])

    invoices = []
    revenue = Decimal("0")

    for order in orders:
        oid = order.get("id")
        cust_id = order.get("customer")
        lines = order.get("lines", [])

        cust = customers.get(cust_id) if isinstance(cust_id, str) else None
        if cust is None:
            cust = customers.get(str(cust_id))
        if cust is None:
            invoices.append({"id": oid, "error": f"unknown customer {cust_id}"})
            continue

        bad_sku = None
        for ln in lines:
            if ln["sku"] not in products:
                bad_sku = ln["sku"]
                break
        if bad_sku is not None:
            invoices.append({"id": oid, "error": f"unknown sku {bad_sku}"})
            continue

        # Compute per-line amounts and units per category.
        line_amounts = []
        units = defaultdict(int)
        for ln in lines:
            prod = products[ln["sku"]]
            qty = int(ln["qty"])
            amount = Decimal(prod["price"]) * qty
            line_amounts.append((prod["category"], amount))
            units[prod["category"]] += qty

        subtotal = Decimal("0")
        discount = Decimal("0")
        discounted_subtotal = Decimal("0")
        for cat, amount in line_amounts:
            subtotal += amount
            cat_disc = Decimal("0")
            if units[cat] >= 3:
                cat_disc = cents(amount * Decimal("0.05"))
            discount += cat_disc
            discounted_subtotal += amount - cat_disc

        tier_disc = cents(discounted_subtotal * TIER_RATE.get(cust["tier"], Decimal("0")))
        discount += tier_disc
        after_discount = discounted_subtotal - tier_disc

        tax = cents(after_discount * TAX_RATE.get(cust["country"], Decimal("0")))
        total = cents(subtotal - discount + tax)

        revenue += total
        invoices.append({
            "id": oid,
            "customer": cust["name"],
            "subtotal": fmt(subtotal),
            "discount": fmt(discount),
            "tax": fmt(tax),
            "total": fmt(total),
        })

    print(json.dumps(invoices, separators=(",", ":")))
    print(f"revenue {fmt(revenue)}")


if __name__ == "__main__":
    main()
