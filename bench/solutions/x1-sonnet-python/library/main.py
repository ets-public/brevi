import sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import re
from catalog import Library, Book, Member, TIERS, to_ord, money
import rules


def cents(t):
    m = re.match(r'^(\d+)(?:\.(\d{1,2}))?$', t)
    if not m:
        return 0
    return int(m.group(1)) * 100 + int((m.group(2) or '0').ljust(2, '0'))


def main():
    lib = Library()
    out = []
    for line in sys.stdin.read().split('\n'):
        p = line.split()
        if len(p) < 3:
            continue
        today = to_ord(p[0])
        out.extend(rules.expire_holds(lib, today))
        op = p[1]
        if op == 'BOOK':
            parts = line.split(None, 4)
            if p[2] in lib.books:
                out.append("duplicate ID")
            else:
                lib.books[p[2]] = Book(p[2], int(p[3]), parts[4] if len(parts) > 4 else "")
                out.append("added " + p[2])
        elif op == 'MEMBER':
            parts = line.split(None, 4)
            if p[2] in lib.members:
                out.append("duplicate ID")
            else:
                name = parts[4] if len(parts) > 4 else ""
                lib.members[p[2]] = Member(p[2], p[3], name)
                out.append("welcome " + name)
        elif op == 'LOAN':
            out.append(rules.loan(lib, today, p[2], p[3]))
        elif op == 'RETURN':
            out.append(rules.give_back(lib, today, p[2], p[3]))
        elif op == 'RESERVE':
            out.append(rules.reserve(lib, today, p[2], p[3]))
        elif op == 'PAY':
            m = lib.members.get(p[2])
            if m is None:
                out.append("refused %s: unknown member" % p[2])
            else:
                m.owes -= cents(p[3])
                out.append("paid %s owes %s" % (p[2], money(m.owes)))
        elif op == 'STATUS':
            m = lib.members.get(p[2])
            if m is None:
                out.append("refused %s: unknown member" % p[2])
            else:
                ls = " ".join(l[0] for l in m.loans) or "-"
                out.append("%s %s owes %s loans: %s" % (m.id, m.name, money(m.owes), ls))
    print("\n".join(out))


main()
