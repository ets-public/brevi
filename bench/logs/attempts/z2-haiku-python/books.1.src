import sys
import re
import datetime

TYPES = ['asset', 'liability', 'equity', 'income', 'expense']
AMOUNT_RE = re.compile(r'([+-]?)(\d+)(?:\.(\d{1,2}))?')
DATE_RE = re.compile(r'\d{4}-\d{2}-\d{2}')


def parse_amount(s):
    m = AMOUNT_RE.fullmatch(s)
    if not m:
        return None
    frac = (m.group(3) or '').ljust(2, '0')
    cents = int(m.group(2)) * 100 + int(frac)
    return -cents if m.group(1) == '-' else cents


def parse_date(s):
    if not DATE_RE.fullmatch(s):
        return None
    try:
        return datetime.date.fromisoformat(s)
    except ValueError:
        return None


def money(cents):
    c = abs(cents)
    return ('-' if cents < 0 else '') + '%d.%02d' % (c // 100, c % 100)


def signed(cents):
    c = abs(cents)
    return ('+' if cents >= 0 else '-') + '%d.%02d' % (c // 100, c % 100)


class Ledger:
    def __init__(self):
        self.types = {}
        self.balance = {}
        self.debits = 0
        self.credits = 0
        self.accepted = 0
        self.rejected = 0
        self.last_date = None
        self.tx = None
        self.out = []

    def say(self, msg):
        self.out.append(msg)

    def discard(self, t):
        if not t['discarded']:
            t['discarded'] = True
            self.rejected += 1

    def close(self):
        t = self.tx
        self.tx = None
        if t is None or t['dead'] or t['discarded']:
            return
        n = t['line']
        if self.last_date is not None and t['date'] < self.last_date:
            self.say('line %d: date goes back' % n)
            self.rejected += 1
            return
        if len(t['postings']) < 2:
            self.say('tx on line %d: needs two postings' % n)
            self.rejected += 1
            return
        total = sum(c for _, c in t['postings'])
        if total != 0:
            self.say('tx on line %d: unbalanced by %s' % (n, signed(total)))
            self.rejected += 1
            return
        self.accepted += 1
        self.last_date = t['date']
        for name, c in t['postings']:
            self.balance[name] = self.balance.get(name, 0) + c
            if c > 0:
                self.debits += c
            elif c < 0:
                self.credits -= c

    def posting(self, n, raw):
        t = self.tx
        if t is None:
            self.say('line %d: bad line' % n)
            return
        if t['dead']:
            return
        tokens = raw.split()
        if len(tokens) != 2 or parse_amount(tokens[1]) is None:
            self.say('line %d: bad line' % n)
            self.discard(t)
            return
        name, amt = tokens
        if name not in self.types:
            self.say('line %d: unknown account %s' % (n, name))
            self.discard(t)
            return
        if t['discarded']:
            return
        t['postings'].append((name, parse_amount(amt)))

    def report(self):
        self.say('trial balance')
        order = sorted(self.types, key=lambda a: (TYPES.index(self.types[a]), a))
        for a in order:
            bal = self.balance.get(a, 0)
            if self.types[a] not in ('asset', 'expense'):
                bal = -bal
            self.say('%s %s %s' % (a, self.types[a], money(bal)))
        self.say('debits %s credits %s' % (money(self.debits), money(self.credits)))

    def line(self, n, raw):
        if raw.strip() == '':
            return
        if raw.startswith('  '):
            self.posting(n, raw)
            return
        self.close()
        tokens = raw.split()
        kw = tokens[0] if tokens else ''
        if kw == 'ACCOUNT' and len(tokens) == 3 and tokens[2] in TYPES:
            if tokens[1] in self.types:
                self.say('line %d: duplicate account' % n)
            else:
                self.types[tokens[1]] = tokens[2]
        elif kw == 'TX':
            date = parse_date(tokens[1]) if len(tokens) >= 3 else None
            if date is None:
                self.say('line %d: bad line' % n)
                self.tx = {'line': n, 'date': None, 'dead': True,
                           'discarded': False, 'postings': []}
            else:
                self.tx = {'line': n, 'date': date, 'dead': False,
                           'discarded': False, 'postings': []}
        elif kw == 'REPORT' and len(tokens) == 1:
            self.report()
        else:
            self.say('line %d: bad line' % n)


def main():
    L = Ledger()
    for n, raw in enumerate(sys.stdin, 1):
        L.line(n, raw.rstrip('\r\n'))
    L.close()
    L.say('%d transactions accepted, %d rejected' % (L.accepted, L.rejected))
    print('\n'.join(L.out))


if __name__ == '__main__':
    main()
