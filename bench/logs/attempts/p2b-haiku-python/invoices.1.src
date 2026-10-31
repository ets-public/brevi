import json
import sys
from decimal import Decimal, ROUND_HALF_EVEN

CENT = Decimal("0.01")
TIER_RATE = {"gold": Decimal("0.10"), "silver": Decimal("0.05"), "basic": Decimal("0")}
TAX_RATE = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}
CAT_RATE = Decimal("0.05")


def money(d):
    return d.quantize(CENT, rounding=ROUND_HALF_EVEN)


def fmt(d):
    return f"{money(d):.2f}"


def main():
    data = json.load(sys.stdin)
    products = data.get("products", {})
    customers = data.get("customers", {})
    orders = data.get("orders", [])

    invoices = []
    revenue = Decimal("0")

    for order in orders:
        oid = order["id"]
        cust_id = order.get("customer")
        cust = customers.get(cust_id) if isinstance(cust_id, str) else None
        if cust is None:
            cust = customers.get(str(cust_id))
        if cust is None:
            invoices.append({"id": oid, "error": f"unknown customer {cust_id}"})
            continue

        lines = order.get("lines", [])
        unknown = None
        for line in lines:
            if line["sku"] not in products:
                unknown = line["sku"]
                break
        if unknown is not None:
            invoices.append({"id": oid, "error": f"unknown sku {unknown}"})
            continue

        cat_amount = {}
        cat_units = {}
        subtotal = Decimal("0")
        for line in lines:
            prod = products[line["sku"]]
            cat = prod["category"]
            qty = line["qty"]
            amount = Decimal(prod["price"]) * qty
            subtotal += amount
            cat_amount[cat] = cat_amount.get(cat, Decimal("0")) + amount
            cat_units[cat] = cat_units.get(cat, 0) + qty

        discount = Decimal("0")
        for cat, amount in cat_amount.items():
            if cat_units[cat] >= 3:
                discount += money(amount * CAT_RATE)

        tier = TIER_RATE.get(cust["tier"], Decimal("0"))
        tier_disc = money((subtotal - discount) * tier)
        discount += tier_disc

        discounted = subtotal - discount
        tax = money(discounted * TAX_RATE.get(cust["country"], Decimal("0")))
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

    print(json.dumps(invoices, separators=(",", ":")))
    print(f"revenue {fmt(revenue)}")


if __name__ == "__main__":
    main()
