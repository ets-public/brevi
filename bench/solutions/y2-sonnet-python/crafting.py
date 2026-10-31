import sys, re

W = r'[^\s,:]+'
Q = r'[1-9]\d*'
RE_STOCK = re.compile(r'STOCK (%s) (%s)' % (W, Q))
RE_RECIPE = re.compile(r'RECIPE (%s) (%s): (%s %s(?:, %s %s)*)' % (W, Q, W, Q, W, Q))
RE_CRAFT = re.compile(r'CRAFT (%s) (%s)' % (W, Q))

stock = {}
recipes = {}

def reaches(start, target, seen):
    if start == target:
        return True
    if start in seen or start not in recipes:
        return False
    seen.add(start)
    return any(reaches(i, target, seen) for i, _ in recipes[start][1])

def main():
    for n, raw in enumerate(sys.stdin.read().split('\n'), 1):
        line = raw.strip()
        if not line:
            continue
        if raw != line:
            print('line %d: bad line' % n)
            continue
        m = RE_STOCK.fullmatch(line)
        if m:
            stock[m.group(1)] = stock.get(m.group(1), 0) + int(m.group(2))
            continue
        m = RE_RECIPE.fullmatch(line)
        if m:
            item, cnt = m.group(1), int(m.group(2))
            ings = []
            for part in m.group(3).split(', '):
                a, b = part.split(' ')
                ings.append((a, int(b)))
            names = [a for a, _ in ings]
            if len(set(names)) != len(names) or item in names:
                print('line %d: bad line' % n)
            elif item in recipes:
                print('line %d: duplicate recipe' % n)
            elif any(reaches(a, item, set()) for a in names):
                print('line %d: cycle' % n)
            else:
                recipes[item] = (cnt, ings)
            continue
        m = RE_CRAFT.fullmatch(line)
        if m:
            craft(n, m.group(1), int(m.group(2)))
            continue
        if line == 'SHOW':
            items = sorted(k for k, v in stock.items() if v > 0)
            if not items:
                print('stock: empty')
            else:
                print('stock:')
                for k in items:
                    print('  %s %d' % (k, stock[k]))
            continue
        print('line %d: bad line' % n)

def craft(n, item, qty):
    if item not in recipes:
        print('line %d: no recipe' % n)
        return
    st = dict(stock)
    short = {}
    batches = {}

    def run_batches(it, b):
        cnt, ings = recipes[it]
        batches[it] = batches.get(it, 0) + b
        for ing, q in ings:
            need(ing, b * q)
        st[it] = st.get(it, 0) + b * cnt

    def need(it, k):
        take = min(st.get(it, 0), k)
        st[it] = st.get(it, 0) - take
        r = k - take
        if r > 0:
            if it in recipes:
                cnt = recipes[it][0]
                b = -(-r // cnt)
                run_batches(it, b)
                st[it] -= r
            else:
                short[it] = short.get(it, 0) + r

    cnt = recipes[item][0]
    run_batches(item, -(-qty // cnt))
    if short:
        print('line %d: short %s' % (n, ', '.join('%s %d' % (k, short[k]) for k in sorted(short))))
        return
    stock.clear()
    stock.update(st)
    print('crafted %d %s (%s)' % (qty, item, ', '.join('%s %d' % (k, batches[k]) for k in sorted(batches))))

main()
