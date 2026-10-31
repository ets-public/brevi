import json
import sys
from decimal import Decimal, ROUND_HALF_EVEN

CENT = Decimal("0.01")


def q(x):
    return x.quantize(CENT, rounding=ROUND_HALF_EVEN)


def fmt(x):
    return f"{q(x):.2f}"


data = json.load(sys.stdin)
products = data["products"]
customers = data["customers"]
orders = data["orders"]

TIER = {"gold": Decimal("0.10"), "silver": Decimal("0.05"), "basic": Decimal("0")}
TAX = {"UK": Decimal("0.20"), "US": Decimal("0.0825")}

results = []
revenue = Decimal("0")

for order in orders:
    oid = order["id"]
    cid = str(order["customer"])
    if cid not in customers:
        results.append({"id": oid, "error": f"unknown customer {cid}"})
        continue
    cust = customers[cid]

    bad = None
    lines = []
    for ln in order["lines"]:
        sku = ln["sku"]
        if sku not in products:
            bad = sku
            break
        p = products[sku]
        lines.append((Decimal(p["price"]), p["category"], int(ln["qty"])))
    if bad is not None:
        results.append({"id": oid, "error": f"unknown sku {bad}"})
        continue

    cat_qty = {}
    for price, cat, qty in lines:
        cat_qty[cat] = cat_qty.get(cat, 0) + qty

    subtotal = Decimal("0")
    cat_disc = Decimal("0")
    for price, cat, qty in lines:
        amt = price * qty
        subtotal += amt
        if cat_qty[cat] >= 3:
            cat_disc += q(amt * Decimal("0.05"))

    after_cat = subtotal - cat_disc
    tier_disc = q(after_cat * TIER.get(cust["tier"], Decimal("0")))
    discount = cat_disc + tier_disc
    discounted = subtotal - discount
    rate = TAX.get(cust["country"], Decimal("0"))
    tax = q(discounted * rate)
    total = discounted + tax
    revenue += total

    results.append({
        "id": oid,
        "customer": cust["name"],
        "subtotal": fmt(subtotal),
        "discount": fmt(discount),
        "tax": fmt(tax),
        "total": fmt(total),
    })

print(json.dumps(results, separators=(",", ":"), ensure_ascii=False))
print(f"revenue {fmt(revenue)}")
