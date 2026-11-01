import sys, re, datetime
TYPES = ['asset', 'liability', 'equity', 'income', 'expense']
AMT = re.compile(r'^[+-]?\d+(\.\d{1,2})?$')
DATE = re.compile(r'^\d{4}-\d{2}-\d{2}$')

def fmt(c):
    s = '-' if c < 0 else ''
    c = abs(c)
    return '%s%d.%02d' % (s, c // 100, c % 100)

def cents(a):
    neg = a.startswith('-')
    a = a.lstrip('+-')
    if '.' in a:
        i, f = a.split('.')
        f = (f + '00')[:2]
    else:
        i, f = a, '00'
    v = int(i) * 100 + int(f)
    return -v if neg else v

def valid_date(d):
    if not DATE.match(d):
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
    if tx is None:
        return
    t = tx
    tx = None
    if t['bad']:
        return
    if t['discard']:
        rejected += 1
        return
    if t['date'] < last:
        print('line %d: date goes back' % t['line'])
        rejected += 1
        return
    if len(t['posts']) < 2:
        print('tx on line %d: needs two postings' % t['line'])
        rejected += 1
        return
    s = sum(a for _, a in t['posts'])
    if s != 0:
        print('tx on line %d: unbalanced by %s' % (t['line'], ('+' if s > 0 else '') + fmt(s)))
        rejected += 1
        return
    accepted += 1
    last = t['date']
    for n, a in t['posts']:
        bal[n] += a
        if a > 0:
            debits += a
        else:
            credits += -a

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
        if tx['bad']:
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
    c = w[0]
    if c == 'ACCOUNT':
        if len(w) != 3 or w[2] not in TYPES:
            print('line %d: bad line' % n)
        elif w[1] in accounts:
            print('line %d: duplicate account' % n)
        else:
            accounts[w[1]] = w[2]
            bal[w[1]] = 0
    elif c == 'TX':
        if len(w) < 3 or not valid_date(w[1]):
            print('line %d: bad line' % n)
            tx = {'bad': True}
        else:
            tx = {'bad': False, 'discard': False, 'line': n, 'date': w[1], 'posts': []}
    elif c == 'REPORT':
        if len(w) != 1:
            print('line %d: bad line' % n)
        else:
            print('trial balance')
            for nm in sorted(accounts, key=lambda x: (TYPES.index(accounts[x]), x)):
                t = accounts[nm]
                b = bal[nm] if t in ('asset', 'expense') else -bal[nm]
                print('%s %s %s' % (nm, t, fmt(b)))
            print('debits %s credits %s' % (fmt(debits), fmt(credits)))
    else:
        print('line %d: bad line' % n)
finish()
print('%d transactions accepted, %d rejected' % (accepted, rejected))
