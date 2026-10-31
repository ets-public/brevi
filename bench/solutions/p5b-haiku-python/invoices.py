import json
import sys
from decimal import Decimal, ROUND_HALF_EVEN

CENT = Decimal("0.01")
TIER_RATE = {"gold": Decimal("0.10"), "silver": Decimal("0.05"), "basic": Decimal("0")}
TAX_RATE = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}
CAT_RATE = Decimal("0.05")


def cents(x):
    return x.quantize(CENT, rounding=ROUND_HALF_EVEN)


def money(x):
    return f"{cents(x):.2f}"


def main():
    data = json.loads(sys.stdin.read())
    products = data.get("products", {})
    customers = data.get("customers", {})
    orders = data.get("orders", [])

    invoices = []
    revenue = Decimal("0")

    for order in orders:
        oid = order["id"]
        cust_id = order["customer"]
        lines = order.get("lines", [])

        if cust_id not in customers:
            invoices.append({"id": oid, "error": f"unknown customer {cust_id}"})
            continue

        bad_sku = None
        for line in lines:
            if line["sku"] not in products:
                bad_sku = line["sku"]
                break
        if bad_sku is not None:
            invoices.append({"id": oid, "error": f"unknown sku {bad_sku}"})
            continue

        cust = customers[cust_id]
        units = {}
        for line in lines:
            cat = products[line["sku"]]["category"]
            units[cat] = units.get(cat, 0) + line["qty"]

        subtotal = Decimal("0")
        cat_discount = Decimal("0")
        for line in lines:
            prod = products[line["sku"]]
            amount = Decimal(prod["price"]) * line["qty"]
            subtotal += amount
            if units[prod["category"]] >= 3:
                cat_discount += cents(amount * CAT_RATE)

        tier_base = subtotal - cat_discount
        tier_discount = cents(tier_base * TIER_RATE[cust["tier"]])
        discount = cat_discount + tier_discount
        discounted = subtotal - discount
        tax = cents(discounted * TAX_RATE.get(cust["country"], Decimal("0")))
        total = discounted + tax

        revenue += total
        invoices.append({
            "id": oid,
            "customer": cust["name"],
            "subtotal": money(subtotal),
            "discount": money(discount),
            "tax": money(tax),
            "total": money(total),
        })

    print(json.dumps(invoices, separators=(",", ":")))
    print(f"revenue {money(revenue)}")


if __name__ == "__main__":
    main()
