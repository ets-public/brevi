from dataclasses import dataclass, field


@dataclass
class Item:
    sku: str
    price: float
    qty: int = 0


@dataclass
class Order:
    oid: int
    items: list = field(default_factory=list)

    def total(self) -> float:
        return sum(i.price * i.qty for i in self.items)
