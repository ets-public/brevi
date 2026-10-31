import sys, re

def main():
    teams = {}
    played = set()
    matches = []
    def st(t):
        return teams.setdefault(t, {'p': 0, 'w': 0, 'd': 0, 'l': 0, 'gf': 0, 'ga': 0, 'pts': 0})
    for n, raw in enumerate(sys.stdin.read().split('\n'), 1):
        t = raw.split()
        if not t:
            continue
        if len(t) != 3 or not re.fullmatch(r'(0|[1-9]\d*)-(0|[1-9]\d*)', t[2]):
            print('line %d: bad line' % n)
            continue
        h, a = t[0], t[1]
        hg, ag = map(int, t[2].split('-'))
        if h == a:
            print('line %d: same team' % n)
            continue
        if (h, a) in played:
            print('line %d: already played' % n)
            continue
        played.add((h, a))
        matches.append((h, a, hg, ag))
        for x, f, g in ((h, hg, ag), (a, ag, hg)):
            s = st(x)
            s['p'] += 1; s['gf'] += f; s['ga'] += g
            if f > g:
                s['w'] += 1; s['pts'] += 3
            elif f == g:
                s['d'] += 1; s['pts'] += 1
            else:
                s['l'] += 1
    if not matches:
        print('no matches')
        return
    def key(t):
        s = teams[t]
        return (-s['pts'], -(s['gf'] - s['ga']), -s['gf'])
    groups = {}
    for t in teams:
        groups.setdefault(key(t), []).append(t)
    order = []
    for k in sorted(groups):
        g = groups[k]
        if len(g) > 1:
            gs = set(g)
            h2h = {t: 0 for t in g}
            for h, a, hg, ag in matches:
                if h in gs and a in gs:
                    if hg > ag:
                        h2h[h] += 3
                    elif hg == ag:
                        h2h[h] += 1; h2h[a] += 1
                    else:
                        h2h[a] += 3
            g = sorted(g, key=lambda t: (-h2h[t], t))
        order += g
    print('Pos Team P W D L GF GA GD Pts')
    for i, t in enumerate(order, 1):
        s = teams[t]
        gd = s['gf'] - s['ga']
        print('%d %s %d %d %d %d %d %d %s %d' % (i, t, s['p'], s['w'], s['d'], s['l'], s['gf'], s['ga'], ('+%d' % gd) if gd > 0 else str(gd), s['pts']))
    mg = max(order, key=lambda t: teams[t]['gf'])
    bd = min(order, key=lambda t: teams[t]['ga'])
    print('most goals: %s (%d)' % (mg, teams[mg]['gf']))
    print('best defence: %s (%d)' % (bd, teams[bd]['ga']))

main()
