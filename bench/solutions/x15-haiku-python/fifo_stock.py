import datetime
import re
import sys
from collections import deque

DATE_RE = re.compile(r"(\d{4})-(\d{2})-(\d{2})")
QTY_RE = re.compile(r"\d+")
PRICE_RE = re.compile(r"(\d+)(?:\.(\d{1,2}))?")


def fmt(cents):
    sign = "-" if cents < 0 else ""
    c = abs(cents)
    return "%s%d.%02d" % (sign, c // 100, c % 100)


def parse_date(s):
    m = DATE_RE.fullmatch(s)
    if not m:
        return None
    try:
        datetime.date(int(m.group(1)), int(m.group(2)), int(m.group(3)))
    except ValueError:
        return None
    return s


def parse_qty(s):
    if not QTY_RE.fullmatch(s):
        return None
    q = int(s)
    return q if q >= 1 else None


def parse_price(s):
    m = PRICE_RE.fullmatch(s)
    if not m:
        return None
    return int(m.group(1)) * 100 + int((m.group(2) or "0").ljust(2, "0"))


class Sku:
    def __init__(self):
        self.lots = deque()  # [qty, unit_cost_cents]
        self.on_hand = 0
        self.cogs = 0
        self.revenue = 0
        self.profit = 0
        self.loss = 0

    def take(self, qty):
        """Remove qty units oldest first; return their total cost."""
        cost = 0
        need = qty
        while need > 0:
            lot = self.lots[0]
            q = min(need, lot[0])
            cost += q * lot[1]
            lot[0] -= q
            need -= q
            if lot[0] == 0:
                self.lots.popleft()
        self.on_hand -= qty
        return cost


def main():
    skus = {}
    months = {}
    latest = None
    out = []

    for n, raw in enumerate(sys.stdin, 1):
        line = raw.strip()
        if not line:
            continue
        parts = line.split()
        ok_form = False
        date = qty = price = None
        if len(parts) >= 4:
            date = parse_date(parts[0])
            op = parts[1]
            sku = parts[2]
            qty = parse_qty(parts[3])
            if date is not None and qty is not None:
                if op == "RECEIVE" and len(parts) == 5:
                    price = parse_price(parts[4])
                    ok_form = price is not None
                elif op == "SHIP" and len(parts) == 5:
                    price = parse_price(parts[4])
                    ok_form = price is not None
                elif op == "LOSS" and len(parts) == 4:
                    ok_form = True
        if not ok_form:
            out.append("line %d: bad line" % n)
            continue
        if latest is not None and date < latest:
            out.append("line %d: date out of order" % n)
            continue

        if op == "RECEIVE":
            latest = date
            st = skus.get(sku)
            if st is None:
                st = skus[sku] = Sku()
            st.lots.append([qty, price])
            st.on_hand += qty
        elif op == "SHIP":
            st = skus.get(sku)
            have = st.on_hand if st is not None else 0
            if have < qty:
                out.append("line %d: short %s need %d have %d" % (n, sku, qty, have))
                continue
            latest = date
            cost = st.take(qty)
            rev = qty * price
            st.cogs += cost
            st.revenue += rev
            st.profit += rev - cost
            m = months.setdefault(date[:7], [0, 0])
            m[0] += rev - cost
        else:  # LOSS
            st = skus.get(sku)
            have = st.on_hand if st is not None else 0
            if have < qty:
                out.append("line %d: short %s need %d have %d" % (n, sku, qty, have))
                continue
            latest = date
            cost = st.take(qty)
            st.loss += cost
            m = months.setdefault(date[:7], [0, 0])
            m[1] += cost

    total_profit = 0
    total_loss = 0
    for name in sorted(skus):
        st = skus[name]
        value = sum(q * c for q, c in st.lots)
        out.append(
            "%s on_hand %d value %s cogs %s revenue %s profit %s loss %s"
            % (name, st.on_hand, fmt(value), fmt(st.cogs), fmt(st.revenue),
               fmt(st.profit), fmt(st.loss))
        )
        total_profit += st.profit
        total_loss += st.loss

    out.append("months")
    for mon in sorted(months):
        p, l = months[mon]
        out.append("  %s profit %s loss %s" % (mon, fmt(p), fmt(l)))

    out.append("total profit %s loss %s net %s"
               % (fmt(total_profit), fmt(total_loss), fmt(total_profit - total_loss)))
    sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
