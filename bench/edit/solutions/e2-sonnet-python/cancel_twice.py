import sys
from dataclasses import dataclass, field


@dataclass
class Product:
    sku: str
    name: str
    price: int  # cents
    stock: int


@dataclass
class Order:
    id: str
    lines: list  # (sku, qty)
    total: int  # cents
    cancelled: bool = False


products = {}
orders = {}


def parse_cents(s):
    whole, _, frac = s.partition(".")
    frac = (frac + "00")[:2]
    return int(whole) * 100 + int(frac)


def money(c):
    return f"{c // 100}.{c % 100:02d}"


def cmd_add(args):
    sku, name, price, qty = args
    if sku in products:
        print(f"error: {sku} exists")
        return
    products[sku] = Product(sku, name, parse_cents(price), int(qty))


def cmd_restock(args):
    sku, qty = args
    p = products.get(sku)
    if p is None:
        print(f"error: unknown product {sku}")
        return
    p.stock += int(qty)


def cmd_order(args):
    oid, rest = args[0], args[1:]
    if oid in orders:
        print(f"error: order {oid} exists")
        return
    lines = []
    for i in range(0, len(rest), 2):
        lines.append((rest[i], int(rest[i + 1])))
    for sku, qty in lines:
        p = products.get(sku)
        if p is None:
            print(f"error: unknown product {sku}")
            return
        if p.stock < qty:
            print(f"error: order {oid}: not enough {sku}")
            return
    total = 0
    for sku, qty in lines:
        p = products[sku]
        p.stock -= qty
        total += p.price * qty
    orders[oid] = Order(oid, lines, total)
    print(f"order {oid}: {money(total)}")


def cmd_cancel(args):
    oid = args[0]
    o = orders.get(oid)
    if o is None:
        print(f"error: unknown order {oid}")
        return
    if o.cancelled:
        print(f"error: order {oid} already cancelled")
        return
    for sku, qty in o.lines:
        products[sku].stock += qty
    o.cancelled = True
    print(f"cancelled {oid}")


def cmd_report(args):
    value = 0
    for sku in sorted(products):
        p = products[sku]
        print(f"{p.sku} {p.name} {p.stock}")
        value += p.price * p.stock
    print(f"stock value {money(value)}")


def cmd_revenue(args):
    total = sum(o.total for o in orders.values() if not o.cancelled)
    print(f"revenue {money(total)}")


COMMANDS = {
    "add": cmd_add,
    "restock": cmd_restock,
    "order": cmd_order,
    "cancel": cmd_cancel,
    "report": cmd_report,
    "revenue": cmd_revenue,
}

for line in sys.stdin:
    words = line.split()
    if not words:
        continue
    f = COMMANDS.get(words[0])
    if f is None:
        print(f"error: unknown command {words[0]}")
        continue
    f(words[1:])
