import sys, re, datetime

ORDER = ['asset', 'liability', 'equity', 'income', 'expense']
AMT = re.compile(r'^[+-]?\d+(\.\d{1,2})?$')


def cents(s):
    neg = s.startswith('-')
    s = s.lstrip('+-')
    if '.' in s:
        a, b = s.split('.')
        b = (b + '00')[:2]
    else:
        a, b = s, '00'
    v = int(a) * 100 + int(b)
    return -v if neg else v


def fmt(v, sign=False):
    s = '-' if v < 0 else ('+' if sign else '')
    v = abs(v)
    return '%s%d.%02d' % (s, v // 100, v % 100)


def valid_date(d):
    if not re.match(r'^\d{4}-\d{2}-\d{2}$', d):
        return False
    try:
        datetime.date(int(d[:4]), int(d[5:7]), int(d[8:]))
        return True
    except ValueError:
        return False


accounts = {}
bal = {}
tx = None
last = ''
accepted = 0
rejected = 0
debits = 0
credits = 0


def finish():
    global tx, last, accepted, rejected, debits, credits
    t = tx
    tx = None
    if t is None or t['badtx']:
        return
    if t['discard']:
        rejected += 1
        return
    if t['date'] < last:
        print('line %d: date goes back' % t['line'])
        rejected += 1
        return
    p = t['posts']
    if len(p) < 2:
        print('tx on line %d: needs two postings' % t['line'])
        rejected += 1
        return
    s = sum(a for _, a in p)
    if s != 0:
        print('tx on line %d: unbalanced by %s' % (t['line'], fmt(s, True)))
        rejected += 1
        return
    accepted += 1
    last = t['date']
    for n, a in p:
        bal[n] += a
        if a > 0:
            debits += a
        else:
            credits += -a


def report():
    print('trial balance')
    for n in sorted(accounts, key=lambda n: (ORDER.index(accounts[n]), n)):
        t = accounts[n]
        b = bal[n] if t in ('asset', 'expense') else -bal[n]
        print('%s %s %s' % (n, t, fmt(b)))
    print('debits %s credits %s' % (fmt(debits), fmt(credits)))


n = 0
for raw in sys.stdin.read().split('\n'):
    n += 1
    line = raw.rstrip('\r')
    if line.strip() == '':
        continue
    if line.startswith('  '):
        w = line.split()
        if tx is None:
            print('line %d: bad line' % n)
            continue
        if tx['badtx']:
            continue
        if len(w) != 2 or not AMT.match(w[1]):
            print('line %d: bad line' % n)
            tx['discard'] = True
            continue
        if w[0] not in accounts:
            print('line %d: unknown account %s' % (n, w[0]))
            tx['discard'] = True
            continue
        tx['posts'].append((w[0], cents(w[1])))
        continue
    finish()
    w = line.split()
    if w[0] == 'ACCOUNT':
        if len(w) != 3 or w[2] not in ORDER:
            print('line %d: bad line' % n)
        elif w[1] in accounts:
            print('line %d: duplicate account' % n)
        else:
            accounts[w[1]] = w[2]
            bal[w[1]] = 0
    elif w[0] == 'TX':
        ok = len(w) >= 3 and valid_date(w[1])
        if not ok:
            print('line %d: bad line' % n)
        tx = {'line': n, 'badtx': not ok, 'date': w[1] if ok else '', 'posts': [], 'discard': False}
    elif w[0] == 'REPORT':
        if len(w) != 1:
            print('line %d: bad line' % n)
        else:
            report()
    else:
        print('line %d: bad line' % n)
finish()
print('%d transactions accepted, %d rejected' % (accepted, rejected))
