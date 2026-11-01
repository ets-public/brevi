import sys

RANKS = '23456789TJQKA'
SUITS = 'SHDC'
CAT_NAMES = ['high card', 'pair', 'two pair', 'three of a kind', 'straight',
             'flush', 'full house', 'four of a kind', 'straight flush']


def valid_card(c):
    return len(c) == 2 and c[0] in RANKS and c[1] in SUITS


def evaluate(cards):
    ranks = [RANKS.index(c[0]) + 2 for c in cards]
    suits = set(c[1] for c in cards)
    counts = {}
    for r in ranks:
        counts[r] = counts.get(r, 0) + 1
    groups = tuple(sorted(counts, key=lambda r: (counts[r], r), reverse=True))
    cnts = sorted(counts.values(), reverse=True)
    flush = len(suits) == 1
    uniq = sorted(counts, reverse=True)
    straight_high = None
    if len(uniq) == 5:
        if uniq[0] - uniq[4] == 4:
            straight_high = uniq[0]
        elif uniq == [14, 5, 4, 3, 2]:
            straight_high = 5
    if straight_high and flush:
        return (8, (straight_high,))
    if cnts[0] == 4:
        return (7, groups)
    if cnts == [3, 2]:
        return (6, groups)
    if flush:
        return (5, groups)
    if straight_high:
        return (4, (straight_high,))
    if cnts[0] == 3:
        return (3, groups)
    if cnts[:2] == [2, 2]:
        return (2, groups)
    if cnts[0] == 2:
        return (1, groups)
    return (0, groups)


def finish(rnd, players):
    print('round %d' % rnd)
    if not players:
        print('no players')
        return
    ordered = sorted(players, key=lambda p: p[0])
    ordered = sorted(ordered, key=lambda p: p[1], reverse=True)
    pos = 0
    prev = None
    for i, (name, key) in enumerate(ordered):
        if key != prev:
            pos = i + 1
            prev = key
        print('%d. %s %s' % (pos, name, CAT_NAMES[key[0]]))


def main():
    rounds = 0
    active = False
    players = []
    names = set()
    dealt = set()
    for n, raw in enumerate(sys.stdin, 1):
        line = raw.strip()
        if line == '':
            if active:
                finish(rounds, players)
                players = []
                names = set()
                dealt = set()
                active = False
            continue
        if not active:
            rounds += 1
            active = True
        tokens = line.split()
        cards = tokens[1:]
        if not (len(tokens) == 6 and all(valid_card(c) for c in cards)
                and len(set(cards)) == 5):
            print('line %d: bad hand' % n)
            continue
        name = tokens[0]
        if name in names:
            print('line %d: duplicate player' % n)
            continue
        clash = None
        for c in cards:
            if c in dealt:
                clash = c
                break
        if clash is not None:
            print('line %d: card %s already dealt' % (n, clash))
            continue
        names.add(name)
        dealt.update(cards)
        players.append((name, evaluate(cards)))
    if active:
        finish(rounds, players)
    print('%d rounds' % rounds)


if __name__ == '__main__':
    main()
