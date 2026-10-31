import sys
import re
from catalog import Book, Member, Library
import rules


def cents(s):
    if "." in s:
        a, b = s.split(".")
        return int(a or 0) * 100 + int((b + "0")[:2])
    return int(s) * 100


def main():
    lib = Library()
    out = []
    for line in sys.stdin.read().split("\n"):
        parts = line.split()
        if len(parts) < 2:
            continue
        try:
            today = rules.parse_date(parts[0])
        except ValueError:
            continue
        out.extend(rules.expire_holds(lib, today))
        op = parts[1]
        a = parts[2:]
        if op == "BOOK" and len(a) >= 3:
            if a[0] in lib.books:
                out.append("duplicate ID")
            else:
                title = line.split(None, 4)[4] if len(line.split(None, 4)) > 4 else ""
                lib.books[a[0]] = Book(a[0], int(a[1]), title)
                out.append("added " + a[0])
        elif op == "MEMBER" and len(a) >= 3:
            if a[0] in lib.members:
                out.append("duplicate ID")
            else:
                name = line.split(None, 4)[4] if len(line.split(None, 4)) > 4 else ""
                lib.members[a[0]] = Member(a[0], a[1], name)
                out.append("welcome " + name)
        elif op == "LOAN" and len(a) == 2:
            out.append(rules.loan(lib, today, a[0], a[1]))
        elif op == "RETURN" and len(a) == 2:
            out.append(rules.give_back(lib, today, a[0], a[1]))
        elif op == "RESERVE" and len(a) == 2:
            out.append(rules.reserve(lib, today, a[0], a[1]))
        elif op == "PAY" and len(a) == 2:
            out.append(rules.pay(lib, a[0], cents(a[1])))
        elif op == "STATUS" and len(a) == 1:
            out.append(rules.status(lib, a[0]))
    print("\n".join(out))


main()
