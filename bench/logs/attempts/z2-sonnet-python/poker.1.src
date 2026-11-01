import sys

RK = '23456789TJQKA'
CATS = ['high card', 'pair', 'two pair', 'three of a kind', 'straight', 'flush',
        'full house', 'four of a kind', 'straight flush']


def card(c):
    return len(c) == 2 and c[0] in RK and c[1] in 'SHDC'


def evaluate(cards):
    rs = sorted((RK.index(c[0]) + 2 for c in cards), reverse=True)
    flush = len(set(c[1] for c in cards)) == 1
    cnt = {}
    for r in rs:
        cnt[r] = cnt.get(r, 0) + 1
    groups = sorted(cnt.items(), key=lambda x: (x[1], x[0]), reverse=True)
    order = [g[0] for g in groups]
    shape = [g[1] for g in groups]
    straight = False
    if len(cnt) == 5:
        if rs[0] - rs[4] == 4:
            straight = True
        elif rs == [14, 5, 4, 3, 2]:
            straight = True
            order = [5]
    if straight and len(order) > 1:
        order = [rs[0]]
    if straight and flush:
        c = 8
    elif shape[0] == 4:
        c = 7
    elif shape[:2] == [3, 2]:
        c = 6
    elif flush:
        c = 5
    elif straight:
        c = 4
    elif shape[0] == 3:
        c = 3
    elif shape[:2] == [2, 2]:
        c = 2
    elif shape[0] == 2:
        c = 1
    else:
        c = 0
    return (c, order)


rounds = 0
players = []
names = set()
dealt = set()
has = False


def flush_round():
    global players, names, dealt, has, rounds
    if not has:
        return
    rounds += 1
    print('round %d' % rounds)
    if not players:
        print('no players')
    else:
        players.sort(key=lambda p: (tuple([-p[1][0]] + [-x for x in p[1][1]]), p[0]))
        prev = None
        for i, (nm, key) in enumerate(players):
            if key != prev:
                pos = i + 1
                prev = key
            print('%d. %s %s' % (pos, nm, CATS[key[0]]))
    players = []
    names = set()
    dealt = set()
    has = False


n = 0
for raw in sys.stdin.read().split('\n'):
    n += 1
    w = raw.split()
    if not w:
        flush_round()
        continue
    has = True
    if len(w) != 6 or not all(card(c) for c in w[1:]) or len(set(w[1:])) != 5:
        print('line %d: bad hand' % n)
        continue
    if w[0] in names:
        print('line %d: duplicate player' % n)
        continue
    bad = [c for c in w[1:] if c in dealt]
    if bad:
        print('line %d: card %s already dealt' % (n, bad[0]))
        continue
    names.add(w[0])
    dealt.update(w[1:])
    players.append((w[0], evaluate(w[1:])))
flush_round()
print('%d rounds' % rounds)
