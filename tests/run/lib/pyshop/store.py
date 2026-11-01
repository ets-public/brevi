from lib.pyshop.models import Item, Order


class Store:
    def __init__(self):
        self.stock = {}
        self.orders = []

    def add(self, sku, price, qty):
        self.stock[sku] = Item(sku, price, qty)

    def order(self, oid, wants):
        o = Order(oid)
        for sku, n in wants:
            it = self.stock[sku]
            if it.qty < n:
                raise ValueError(f"short {sku}")
            it.qty -= n
            o.items.append(Item(sku, it.price, n))
        self.orders.append(o)
        return o


if __name__ == "__main__":
    s = Store()
    s.add("x", 1.0, 1)

    print("self-test", len(s.stock))
