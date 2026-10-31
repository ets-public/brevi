import re
import sys

SCORE_RE = re.compile(r'(0|[1-9][0-9]*)-(0|[1-9][0-9]*)')


def main():
    data = sys.stdin.read()
    lines = data.split('\n')
    teams = {}
    played = set()
    results = []  # (home, away, hg, ag)
    out = []

    for lineno, raw in enumerate(lines, 1):
        tok = raw.split()
        if not tok:
            continue
        m = SCORE_RE.fullmatch(tok[2]) if len(tok) == 3 else None
        if len(tok) != 3 or m is None:
            out.append('line %d: bad line' % lineno)
            continue
        home, away = tok[0], tok[1]
        hg, ag = int(m.group(1)), int(m.group(2))
        if home == away:
            out.append('line %d: same team' % lineno)
            continue
        if (home, away) in played:
            out.append('line %d: already played' % lineno)
            continue
        played.add((home, away))
        for t in (home, away):
            if t not in teams:
                teams[t] = {'P': 0, 'W': 0, 'D': 0, 'L': 0, 'GF': 0, 'GA': 0}
        for t, gf, ga in ((home, hg, ag), (away, ag, hg)):
            s = teams[t]
            s['P'] += 1
            s['GF'] += gf
            s['GA'] += ga
            if gf > ga:
                s['W'] += 1
            elif gf == ga:
                s['D'] += 1
            else:
                s['L'] += 1
        results.append((home, away, hg, ag))

    if not results:
        out.append('no matches')
        print('\n'.join(out))
        return

    def pts(t):
        s = teams[t]
        return s['W'] * 3 + s['D']

    def gd(t):
        s = teams[t]
        return s['GF'] - s['GA']

    order = sorted(teams, key=lambda t: (-pts(t), -gd(t), -teams[t]['GF'], t))

    # Re-order teams level on points, goal difference and goals for,
    # using only the matches between those teams.
    i = 0
    final = []
    while i < len(order):
        j = i
        key = (pts(order[i]), gd(order[i]), teams[order[i]]['GF'])
        while j < len(order) and (pts(order[j]), gd(order[j]), teams[order[j]]['GF']) == key:
            j += 1
        group = order[i:j]
        if len(group) > 1:
            gs = set(group)
            h2h = {t: 0 for t in group}
            for home, away, hg, ag in results:
                if home in gs and away in gs:
                    if hg > ag:
                        h2h[home] += 3
                    elif hg < ag:
                        h2h[away] += 3
                    else:
                        h2h[home] += 1
                        h2h[away] += 1
            group = sorted(group, key=lambda t: (-h2h[t], t))
        final.extend(group)
        i = j

    out.append('Pos Team P W D L GF GA GD Pts')
    for idx, t in enumerate(final, 1):
        s = teams[t]
        g = gd(t)
        gs = ('+%d' % g) if g > 0 else ('%d' % g)
        out.append('%d %s %d %d %d %d %d %d %s %d' % (
            idx, t, s['P'], s['W'], s['D'], s['L'], s['GF'], s['GA'], gs, pts(t)))

    best_att = max(final, key=lambda t: teams[t]['GF'])
    best_def = min(final, key=lambda t: teams[t]['GA'])
    out.append('most goals: %s (%d)' % (best_att, teams[best_att]['GF']))
    out.append('best defence: %s (%d)' % (best_def, teams[best_def]['GA']))
    print('\n'.join(out))


if __name__ == '__main__':
    main()
