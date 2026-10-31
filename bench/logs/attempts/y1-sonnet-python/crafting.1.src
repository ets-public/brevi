import sys, re

W = r'[A-Za-z0-9_]+'
Q = r'[1-9][0-9]*'
stock = {}
recipes = {}
out = []

def reach(a, target, seen):
    if a == target: return True
    if a in seen or a not in recipes: return False
    seen.add(a)
    return any(reach(i, target, seen) for i, _ in recipes[a][1])

def craft(item, qty, st, short, bat):
    n, ings = recipes[item]
    b = -(-qty // n)
    bat[item] = bat.get(item, 0) + b
    for ing, q in ings:
        need = b * q
        take = min(st.get(ing, 0), need)
        st[ing] = st.get(ing, 0) - take
        rest = need - take
        if rest:
            if ing in recipes:
                made = craft(ing, rest, st, short, bat)
                st[ing] = st.get(ing, 0) + made - rest
            else:
                short[ing] = short.get(ing, 0) + rest
    return b * n

for ln, raw in enumerate(sys.stdin.read().split('\n'), 1):
    line = raw.strip()
    if not line: continue
    m = re.fullmatch(r'STOCK (%s) (%s)' % (W, Q), line)
    if m:
        stock[m.group(1)] = stock.get(m.group(1), 0) + int(m.group(2)); continue
    m = re.fullmatch(r'RECIPE (%s) (%s): (.+)' % (W, Q), line)
    if m:
        item, n, rest = m.group(1), int(m.group(2)), m.group(3)
        ings = []
        ok = True
        for part in rest.split(', '):
            mm = re.fullmatch(r'(%s) (%s)' % (W, Q), part)
            if not mm or mm.group(1) == item or any(mm.group(1) == i for i, _ in ings):
                ok = False; break
            ings.append((mm.group(1), int(mm.group(2))))
        if not ok:
            out.append("line %d: bad line" % ln); continue
        if item in recipes:
            out.append("line %d: duplicate recipe" % ln); continue
        if any(reach(i, item, set()) for i, _ in ings):
            out.append("line %d: cycle" % ln); continue
        recipes[item] = (n, ings); continue
    m = re.fullmatch(r'CRAFT (%s) (%s)' % (W, Q), line)
    if m:
        item, qty = m.group(1), int(m.group(2))
        if item not in recipes:
            out.append("line %d: no recipe" % ln); continue
        st = dict(stock); short = {}; bat = {}
        made = craft(item, qty, st, short, bat)
        if short:
            out.append("line %d: short %s" % (ln, ", ".join("%s %d" % (k, short[k]) for k in sorted(short))))
        else:
            st[item] = st.get(item, 0) + made
            stock = st
            out.append("crafted %d %s (%s)" % (qty, item, ", ".join("%s %d" % (k, bat[k]) for k in sorted(bat))))
        continue
    if line == 'SHOW':
        items = [k for k in sorted(stock) if stock[k] > 0]
        out.append("stock:" + ("".join("\n  %s %d" % (k, stock[k]) for k in items) if items else " empty"))
        continue
    out.append("line %d: bad line" % ln)
print('\n'.join(out))
