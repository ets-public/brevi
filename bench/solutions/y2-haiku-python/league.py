import sys
import re

SCORE_RE = re.compile(r'(0|[1-9][0-9]*)-(0|[1-9][0-9]*)')


def main():
    lines = sys.stdin.read().splitlines()
    matches = []      # (home, away, hg, ag)
    played = set()    # (home, away)
    teams = {}        # name -> stats dict, in join order
    for n, raw in enumerate(lines, 1):
        tok = raw.split()
        if not tok:
            continue
        if len(tok) != 3 or not SCORE_RE.fullmatch(tok[2]):
            print(f"line {n}: bad line")
            continue
        home, away = tok[0], tok[1]
        hg, ag = (int(x) for x in tok[2].split('-'))
        if home == away:
            print(f"line {n}: same team")
            continue
        if (home, away) in played:
            print(f"line {n}: already played")
            continue
        played.add((home, away))
        matches.append((home, away, hg, ag))
        for t in (home, away):
            if t not in teams:
                teams[t] = {'P': 0, 'W': 0, 'D': 0, 'L': 0,
                            'GF': 0, 'GA': 0, 'Pts': 0}
    if not matches:
        print("no matches")
        return

    for home, away, hg, ag in matches:
        h, a = teams[home], teams[away]
        h['P'] += 1
        a['P'] += 1
        h['GF'] += hg
        h['GA'] += ag
        a['GF'] += ag
        a['GA'] += hg
        if hg > ag:
            h['W'] += 1
            a['L'] += 1
            h['Pts'] += 3
        elif hg < ag:
            a['W'] += 1
            h['L'] += 1
            a['Pts'] += 3
        else:
            h['D'] += 1
            a['D'] += 1
            h['Pts'] += 1
            a['Pts'] += 1

    def overall(t):
        s = teams[t]
        return (s['Pts'], s['GF'] - s['GA'], s['GF'])

    # group teams that are level on points, goal difference and goals for
    groups = {}
    for t in teams:
        groups.setdefault(overall(t), []).append(t)

    order = []
    for key in sorted(groups, reverse=True):
        group = groups[key]
        if len(group) == 1:
            order.extend(group)
            continue
        members = set(group)
        mini = {t: 0 for t in group}
        for home, away, hg, ag in matches:
            if home in members and away in members:
                if hg > ag:
                    mini[home] += 3
                elif hg < ag:
                    mini[away] += 3
                else:
                    mini[home] += 1
                    mini[away] += 1
        order.extend(sorted(group, key=lambda t: (-mini[t], t)))

    out = ["Pos Team P W D L GF GA GD Pts"]
    for pos, t in enumerate(order, 1):
        s = teams[t]
        gd = s['GF'] - s['GA']
        gd_str = f"+{gd}" if gd > 0 else str(gd)
        out.append(f"{pos} {t} {s['P']} {s['W']} {s['D']} {s['L']} "
                   f"{s['GF']} {s['GA']} {gd_str} {s['Pts']}")

    top_scorer = max(order, key=lambda t: teams[t]['GF'])
    best_def = min(order, key=lambda t: teams[t]['GA'])
    out.append(f"most goals: {top_scorer} ({teams[top_scorer]['GF']})")
    out.append(f"best defence: {best_def} ({teams[best_def]['GA']})")
    print("\n".join(out))


if __name__ == "__main__":
    main()
