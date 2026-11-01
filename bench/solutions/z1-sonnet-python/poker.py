import sys
R = '23456789TJQKA'
S = 'SHDC'
CATS = ['high card', 'pair', 'two pair', 'three of a kind', 'straight', 'flush', 'full house', 'four of a kind', 'straight flush']

def evaluate(cards):
    rs = sorted((R.index(c[0]) + 2 for c in cards), reverse=True)
    flush = len(set(c[1] for c in cards)) == 1
    cnt = {}
    for r in rs:
        cnt[r] = cnt.get(r, 0) + 1
    groups = sorted(cnt.items(), key=lambda x: (-x[1], -x[0]))
    shape = [g[1] for g in groups]
    order = [g[0] for g in groups]
    straight = None
    if len(cnt) == 5:
        if rs[0] - rs[4] == 4:
            straight = rs[0]
        elif rs == [14, 5, 4, 3, 2]:
            straight = 5
    if straight and flush:
        return (8, [straight])
    if shape == [4, 1]:
        return (7, order)
    if shape == [3, 2]:
        return (6, order)
    if flush:
        return (5, rs)
    if straight:
        return (4, [straight])
    if shape == [3, 1, 1]:
        return (3, order)
    if shape == [2, 2, 1]:
        return (2, order)
    if shape == [2, 1, 1, 1]:
        return (1, order)
    return (0, rs)

rounds = 0
players = []
names = set()
dealt = set()
nlines = 0

def end_round():
    global players, names, dealt, nlines, rounds
    if nlines == 0:
        return
    rounds += 1
    print('round %d' % rounds)
    if not players:
        print('no players')
    else:
        ps = sorted(players, key=lambda p: (tuple([-p[0][0]] + [-x for x in p[0][1]]), p[1]))
        pos = 0
        prev = None
        for i, (k, nm) in enumerate(ps):
            if k != prev:
                pos = i + 1
                prev = k
            print('%d. %s %s' % (pos, nm, CATS[k[0]]))
    players = []
    names = set()
    dealt = set()
    nlines = 0

n = 0
for raw in sys.stdin.read().split('\n'):
    n += 1
    w = raw.split()
    if not w:
        end_round()
        continue
    nlines += 1
    cards = w[1:]
    ok = len(w) == 6 and all(len(c) == 2 and c[0] in R and c[1] in S for c in cards) and len(set(cards)) == 5
    if not ok:
        print('line %d: bad hand' % n)
        continue
    if w[0] in names:
        print('line %d: duplicate player' % n)
        continue
    d = [c for c in cards if c in dealt]
    if d:
        print('line %d: card %s already dealt' % (n, d[0]))
        continue
    names.add(w[0])
    dealt.update(cards)
    players.append((evaluate(cards), w[0]))
end_round()
print('%d rounds' % rounds)
