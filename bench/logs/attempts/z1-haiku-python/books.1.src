import sys
import re
from datetime import date
from decimal import Decimal

TYPES = ["asset", "liability", "equity", "income", "expense"]
AMOUNT_RE = re.compile(r"^[+-]?(\d+(\.\d{1,2})?|\.\d{1,2})$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def parse_amount(s):
    if not AMOUNT_RE.match(s):
        return None
    return Decimal(s)


def valid_date(s):
    if not DATE_RE.match(s):
        return None
    try:
        y, m, d = map(int, s.split("-"))
        date(y, m, d)
        return s
    except ValueError:
        return None


def fmt(x):
    return "{:.2f}".format(x.quantize(Decimal("0.01")) + Decimal(0))


def fmt_signed(x):
    q = x.quantize(Decimal("0.01"))
    s = "{:.2f}".format(abs(q))
    return ("-" if q < 0 else "+") + s


def main():
    accounts = {}  # name -> type
    balances = {}  # name -> Decimal sum of postings
    accepted = 0
    rejected = 0
    last_date = None
    debits = Decimal(0)
    credits = Decimal(0)

    tx = None
    # tx: {'line', 'date', 'bad', 'discard', 'postings'}

    def finish_tx():
        nonlocal tx, accepted, rejected, last_date, debits, credits
        if tx is None:
            return
        t = tx
        tx = None
        if t["bad"]:
            return
        if t["discard"]:
            rejected += 1
            return
        if last_date is not None and t["date"] < last_date:
            print("line %d: date goes back" % t["line"])
            rejected += 1
            return
        if len(t["postings"]) < 2:
            print("tx on line %d: needs two postings" % t["line"])
            rejected += 1
            return
        total = sum((a for _, a in t["postings"]), Decimal(0))
        if total != 0:
            print("tx on line %d: unbalanced by %s" % (t["line"], fmt_signed(total)))
            rejected += 1
            return
        for name, a in t["postings"]:
            balances[name] = balances.get(name, Decimal(0)) + a
            if a > 0:
                debits += a
            elif a < 0:
                credits += a
        last_date = t["date"]
        accepted += 1

    lineno = 0
    for raw in sys.stdin:
        lineno += 1
        line = raw.rstrip("\r\n")

        if line.strip() == "":
            continue

        if line.startswith("  "):
            if tx is None:
                print("line %d: bad line" % lineno)
                continue
            if tx["bad"]:
                continue
            parts = line.split()
            if len(parts) != 2:
                print("line %d: bad line" % lineno)
                tx["discard"] = True
                continue
            name, amt_s = parts
            amt = parse_amount(amt_s)
            if amt is None:
                print("line %d: bad line" % lineno)
                tx["discard"] = True
                continue
            if name not in accounts:
                print("line %d: unknown account %s" % (lineno, name))
                tx["discard"] = True
                continue
            tx["postings"].append((name, amt))
            continue

        parts = line.split()
        kw = parts[0]

        if kw == "ACCOUNT":
            finish_tx()
            if len(parts) != 3 or parts[2] not in TYPES:
                print("line %d: bad line" % lineno)
                continue
            name, typ = parts[1], parts[2]
            if name in accounts:
                print("line %d: duplicate account" % lineno)
                continue
            accounts[name] = typ
            balances[name] = Decimal(0)
            continue

        if kw == "TX":
            finish_tx()
            if len(parts) < 3 or valid_date(parts[1]) is None:
                print("line %d: bad line" % lineno)
                tx = {"line": lineno, "date": "", "bad": True,
                      "discard": False, "postings": []}
                continue
            tx = {"line": lineno, "date": parts[1], "bad": False,
                  "discard": False, "postings": []}
            continue

        if kw == "REPORT" and len(parts) == 1:
            finish_tx()
            print("trial balance")
            order = {t: i for i, t in enumerate(TYPES)}
            for name in sorted(accounts, key=lambda n: (order[accounts[n]], n)):
                typ = accounts[name]
                s = balances[name]
                bal = s if typ in ("asset", "expense") else -s
                print("%s %s %s" % (name, typ, fmt(bal)))
            print("debits %s credits %s" % (fmt(debits), fmt(-credits)))
            continue

        finish_tx()
        print("line %d: bad line" % lineno)

    finish_tx()
    print("%d transactions accepted, %d rejected" % (accepted, rejected))


if __name__ == "__main__":
    main()
