import sys

from catalog import Library, Book, Member, parse_date, to_cents
import rules


def main():
    lib = Library()
    out = []
    for raw in sys.stdin.read().splitlines():
        f = raw.split(None, 4)
        if not f:
            continue
        today = parse_date(f[0])
        out.extend(rules.expire_holds(lib, today))
        op = f[1] if len(f) > 1 else ''
        if op == 'BOOK' and len(f) >= 4:
            bid = f[2]
            if bid in lib.books:
                out.append("duplicate ID")
            else:
                title = f[4] if len(f) > 4 else ''
                lib.books[bid] = Book(bid, title, int(f[3]))
                out.append("added %s" % bid)
        elif op == 'MEMBER' and len(f) >= 4:
            mid = f[2]
            if mid in lib.members:
                out.append("duplicate ID")
            else:
                name = f[4] if len(f) > 4 else ''
                lib.members[mid] = Member(mid, name, f[3])
                out.append("welcome %s" % name)
        elif op == 'LOAN':
            out.append(rules.loan(lib, today, f[2], f[3]))
        elif op == 'RETURN':
            out.append(rules.ret(lib, today, f[2], f[3]))
        elif op == 'RESERVE':
            out.append(rules.reserve(lib, today, f[2], f[3]))
        elif op == 'PAY':
            out.append(rules.pay(lib, f[2], to_cents(f[3])))
        elif op == 'STATUS':
            out.append(rules.status(lib, f[2]))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == '__main__':
    main()
