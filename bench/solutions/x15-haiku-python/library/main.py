import re
import sys

from catalog import TIERS, Book, Library, Member, day_of, money
from rules import expire_holds, loan, pay, reserve, ret, status

AMOUNT_RE = re.compile(r"(\d+)(?:\.(\d{1,2}))?")


def parse_amount(text):
    m = AMOUNT_RE.fullmatch(text)
    if not m:
        return None
    return int(m.group(1)) * 100 + int((m.group(2) or "0").ljust(2, "0"))


def handle(lib, today, parts, line):
    op = parts[1]
    out = []
    if op == "BOOK" and len(parts) >= 4:
        bid = parts[2]
        if not parts[3].isdigit() or int(parts[3]) < 1:
            return out
        title = parts[4] if len(parts) > 4 else ""
        if bid in lib.books:
            return ["duplicate ID"]
        lib.books[bid] = Book(bid, title, int(parts[3]))
        return ["added %s" % bid]
    if op == "MEMBER" and len(parts) >= 4:
        mid = parts[2]
        tier = parts[3]
        if tier not in TIERS:
            return out
        name = parts[4] if len(parts) > 4 else ""
        if mid in lib.members:
            return ["duplicate ID"]
        lib.members[mid] = Member(mid, name, tier)
        return ["welcome %s" % name]
    if op == "LOAN" and len(parts) == 4:
        return [loan(lib, today, parts[2], parts[3])]
    if op == "RETURN" and len(parts) == 4:
        return [ret(lib, today, parts[2], parts[3])]
    if op == "RESERVE" and len(parts) == 4:
        return [reserve(lib, parts[2], parts[3])]
    if op == "PAY" and len(parts) == 4:
        amount = parse_amount(parts[3])
        if amount is None:
            return out
        return [pay(lib, parts[2], amount)]
    if op == "STATUS" and len(parts) == 3:
        return [status(lib, parts[2])]
    return out


def main():
    lib = Library()
    out = []
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        parts = line.split(maxsplit=4)
        if len(parts) < 2:
            continue
        today = day_of(parts[0])
        if today is None:
            continue
        out.extend(expire_holds(lib, today))
        out.extend(handle(lib, today, parts, line))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
