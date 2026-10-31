import re
import sys

NUM = r'(0|[1-9][0-9]*)'
NAME = r'\w+'


def ceil_div(a, b):
    return -(-a // b)


def produce(item, qty, recipes, st, short, log):
    n_out, ings = recipes[item]
    b = ceil_div(qty, n_out)
    log[item] = log.get(item, 0) + b
    for ing, q in ings:
        need = b * q
        take = min(st.get(ing, 0), need)
        st[ing] = st.get(ing, 0) - take
        rem = need - take
        if rem > 0:
            if ing in recipes:
                produce(ing, rem, recipes, st, short, log)
                st[ing] -= rem
            else:
                short[ing] = short.get(ing, 0) + rem
    st[item] = st.get(item, 0) + b * n_out


def reaches(start_items, target, recipes):
    seen = set()
    stack = list(start_items)
    while stack:
        x = stack.pop()
        if x == target:
            return True
        if x in seen:
            continue
        seen.add(x)
        if x in recipes:
            stack.extend(i for i, _ in recipes[x][1])
    return False


def main():
    data = sys.stdin.read()
    lines = data.split('\n')
    stock = {}
    recipes = {}
    out = []

    for lineno, raw in enumerate(lines, 1):
        s = raw.strip()
        if not s:
            continue

        def bad():
            out.append('line %d: bad line' % lineno)

        if s == 'SHOW':
            if any(v > 0 for v in stock.values()):
                out.append('stock:')
                for k in sorted(stock):
                    if stock[k] > 0:
                        out.append('  %s %d' % (k, stock[k]))
            else:
                out.append('stock: empty')
            continue

        m = re.fullmatch(r'STOCK (' + NAME + r') ' + NUM, s)
        if m:
            q = int(m.group(2))
            if q < 1:
                bad()
                continue
            stock[m.group(1)] = stock.get(m.group(1), 0) + q
            continue

        m = re.fullmatch(r'CRAFT (' + NAME + r') ' + NUM, s)
        if m:
            item, q = m.group(1), int(m.group(2))
            if q < 1:
                bad()
                continue
            if item not in recipes:
                out.append('line %d: no recipe' % lineno)
                continue
            st = dict(stock)
            short = {}
            log = {}
            produce(item, q, recipes, st, short, log)
            if short:
                parts = ', '.join('%s %d' % (k, short[k]) for k in sorted(short))
                out.append('line %d: short %s' % (lineno, parts))
                continue
            stock.clear()
            stock.update(st)
            parts = ', '.join('%s %d' % (k, log[k]) for k in sorted(log))
            out.append('crafted %d %s (%s)' % (q, item, parts))
            continue

        m = re.fullmatch(r'RECIPE (' + NAME + r') (0|[1-9][0-9]*): (.+)', s)
        if m:
            item = m.group(1)
            n_out = int(m.group(2))
            if n_out < 1:
                bad()
                continue
            ings = []
            ok = True
            for part in m.group(3).split(', '):
                pm = re.fullmatch(r'(' + NAME + r') ' + NUM, part)
                if not pm or int(pm.group(2)) < 1 or pm.group(1) == item:
                    ok = False
                    break
                if any(name == pm.group(1) for name, _ in ings):
                    ok = False
                    break
                ings.append((pm.group(1), int(pm.group(2))))
            if not ok:
                bad()
                continue
            if item in recipes:
                out.append('line %d: duplicate recipe' % lineno)
                continue
            if reaches([i for i, _ in ings], item, recipes):
                out.append('line %d: cycle' % lineno)
                continue
            recipes[item] = (n_out, ings)
            continue

        bad()

    print('\n'.join(out))


if __name__ == '__main__':
    main()
