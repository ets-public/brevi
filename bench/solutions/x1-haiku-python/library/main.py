import datetime
import sys

import rules
from catalog import Book, Member, Library


def main():
    lib = Library()
    out = []
    for raw in sys.stdin.read().split('\n'):
        line = raw.strip()
        if not line:
            continue
        parts = line.split()
        date = datetime.date.fromisoformat(parts[0])
        out.extend(rules.expire_holds(lib, date))
        cmd = parts[1]

        if cmd == 'BOOK':
            sp = line.split(None, 4)
            bid = sp[2]
            if bid in lib.books:
                out.append('duplicate ID')
            else:
                title = sp[4] if len(sp) > 4 else ''
                lib.books[bid] = Book(bid, title, int(sp[3]))
                out.append('added %s' % bid)
        elif cmd == 'MEMBER':
            sp = line.split(None, 4)
            mid, tier = sp[2], sp[3]
            if mid in lib.members:
                out.append('duplicate ID')
            else:
                name = sp[4] if len(sp) > 4 else ''
                lib.members[mid] = Member(mid, name, tier)
                out.append('welcome %s' % name)
        elif cmd == 'LOAN':
            out.append(rules.loan(lib, date, parts[2], parts[3]))
        elif cmd == 'RETURN':
            out.append(rules.return_book(lib, date, parts[2], parts[3]))
        elif cmd == 'RESERVE':
            out.append(rules.reserve(lib, date, parts[2], parts[3]))
        elif cmd == 'PAY':
            out.append(rules.pay(lib, parts[2], rules.to_cents(parts[3])))
        elif cmd == 'STATUS':
            out.append(rules.status(lib, parts[2]))
    sys.stdout.write('\n'.join(out) + ('\n' if out else ''))


main()
