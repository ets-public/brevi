import sys
import datetime
from catalog import Library
import rules


def handle(lib, line):
    t = line.split(None, 4)
    if len(t) < 2:
        return []
    try:
        day = datetime.date.fromisoformat(t[0]).toordinal()
    except ValueError:
        return []
    out = rules.expire_holds(lib, day)
    op = t[1]
    a = t[2:]
    if op == 'BOOK' and len(a) >= 3:
        out += rules.add_book(lib, a[0], int(a[1]), a[2])
    elif op == 'MEMBER' and len(a) >= 3:
        out += rules.add_member(lib, a[0], a[1], a[2])
    elif op == 'LOAN' and len(a) >= 2:
        out += rules.loan(lib, day, a[0], a[1])
    elif op == 'RETURN' and len(a) >= 2:
        out += rules.give_back(lib, day, a[0], a[1])
    elif op == 'RESERVE' and len(a) >= 2:
        out += rules.reserve(lib, a[0], a[1])
    elif op == 'PAY' and len(a) >= 2:
        out += rules.pay(lib, a[0], a[1])
    elif op == 'STATUS' and len(a) >= 1:
        out += rules.status(lib, a[0])
    return out


def main():
    lib = Library()
    out = []
    for line in sys.stdin.read().split('\n'):
        if line.strip():
            out += handle(lib, line)
    print('\n'.join(out))


main()
