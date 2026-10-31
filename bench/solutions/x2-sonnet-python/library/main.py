import sys
from catalog import Book, Member, Library
import rules


def handle(lib, line):
    t = line.split(None, 4)
    date, op = t[0], t[1] if len(t) > 1 else ""
    out = rules.expire_holds(lib, date)
    a = t[2:]
    if op == "BOOK" and len(a) >= 3:
        if a[0] in lib.books:
            out.append("duplicate ID")
        else:
            lib.books[a[0]] = Book(a[0], int(a[1]), a[2])
            out.append("added " + a[0])
    elif op == "MEMBER" and len(a) >= 3:
        if a[0] in lib.members:
            out.append("duplicate ID")
        else:
            lib.members[a[0]] = Member(a[0], a[1], a[2])
            out.append("welcome " + a[2])
    elif op == "LOAN" and len(a) >= 2:
        out.append(rules.loan(lib, date, a[0], a[1]))
    elif op == "RETURN" and len(a) >= 2:
        out.append(rules.give_back(lib, date, a[0], a[1]))
    elif op == "RESERVE" and len(a) >= 2:
        out.append(rules.reserve(lib, date, a[0], a[1]))
    elif op == "PAY" and len(a) >= 2:
        out.append(rules.pay(lib, a[0], a[1]))
    elif op == "STATUS" and len(a) >= 1:
        out.append(rules.status(lib, a[0]))
    return out


def main():
    lib = Library()
    out = []
    for line in sys.stdin.read().split("\n"):
        if line.strip():
            out.extend(handle(lib, line.strip()))
    if out:
        print("\n".join(out))


main()
