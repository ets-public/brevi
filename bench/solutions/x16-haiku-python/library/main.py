import sys
import re
import datetime

from catalog import Library, Book, Member, TIERS
import rules

AMOUNT_RE = re.compile(r'([0-9]+)(?:\.([0-9]{1,2}))?\Z')


def parse_amount(s):
    m = AMOUNT_RE.match(s)
    if not m:
        return None
    return int(m.group(1)) * 100 + int((m.group(2) or '').ljust(2, '0'))


def main():
    lib = Library()
    out = []

    for raw in sys.stdin.read().split('\n'):
        line = raw.strip()
        if not line:
            continue
        parts = line.split(None, 4)
        if len(parts) < 2:
            continue
        today = datetime.date.fromisoformat(parts[0])
        cmd = parts[1]

        out.extend(rules.expire_holds(lib, today))

        if cmd == 'BOOK' and len(parts) >= 4:
            bid = parts[2]
            title = parts[4] if len(parts) == 5 else ''
            if bid in lib.books:
                out.append('duplicate ID')
            else:
                lib.books[bid] = Book(bid, title, int(parts[3]))
                out.append('added %s' % bid)

        elif cmd == 'MEMBER' and len(parts) >= 4:
            mid = parts[2]
            tier = parts[3] if parts[3] in TIERS else 'basic'
            name = parts[4] if len(parts) == 5 else ''
            if mid in lib.members:
                out.append('duplicate ID')
            else:
                lib.members[mid] = Member(mid, name, tier)
                out.append('welcome %s' % name)

        elif cmd == 'LOAN' and len(parts) == 4:
            out.append(rules.loan(lib, today, parts[2], parts[3]))

        elif cmd == 'RETURN' and len(parts) == 4:
            out.append(rules.ret(lib, today, parts[2], parts[3]))

        elif cmd == 'RESERVE' and len(parts) == 4:
            out.append(rules.reserve(lib, today, parts[2], parts[3]))

        elif cmd == 'PAY' and len(parts) == 4:
            amount = parse_amount(parts[3])
            if amount is not None:
                out.append(rules.pay(lib, parts[2], amount))

        elif cmd == 'STATUS' and len(parts) == 3:
            out.append(rules.status(lib, parts[2]))

    if out:
        sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
