import json
import sys
from decimal import Decimal, ROUND_HALF_EVEN

CENT = Decimal("0.01")
TIER = {"gold": Decimal("0.10"), "silver": Decimal("0.05"), "basic": Decimal("0")}
TAX = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}


def q(x):
    return x.quantize(CENT, rounding=ROUND_HALF_EVEN)


def fmt(x):
    return f"{q(x):.2f}"


def main():
    data = json.loads(sys.stdin.read())
    products = data["products"]
    customers = data["customers"]
    invoices = []
    revenue = Decimal("0")

    for order in data["orders"]:
        oid = order["id"]
        cust = customers.get(str(order["customer"]))
        if cust is None:
            invoices.append({"id": oid, "error": f"unknown customer {order['customer']}"})
            continue
        lines = order["lines"]
        bad = None
        for line in lines:
            if line["sku"] not in products:
                bad = line["sku"]
                break
        if bad is not None:
            invoices.append({"id": oid, "error": f"unknown sku {bad}"})
            continue

        units = {}
        priced = []
        for line in lines:
            p = products[line["sku"]]
            qty = line["qty"]
            amount = Decimal(p["price"]) * qty
            cat = p["category"]
            units[cat] = units.get(cat, 0) + qty
            priced.append((amount, cat))

        subtotal = Decimal("0")
        discount = Decimal("0")
        for amount, cat in priced:
            subtotal += amount
            if units[cat] >= 3:
                discount += q(amount * Decimal("0.05"))
        # tier discount applies after the category discounts
        tier_d = q((subtotal - discount) * TIER.get(cust["tier"], Decimal("0")))
        discount += tier_d
        discounted = subtotal - discount
        tax = q(discounted * TAX.get(cust["country"], Decimal("0")))
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
