import sys

class Bad(Exception):
    pass

def parse(p):
    n = len(p)
    i = 0
    sa = ea = False
    if n and p[0] == '^':
        sa = True
        i = 1
    items = []  # [matcher, min, max]
    last_rep = False
    while i < n:
        c = p[i]
        if c == '$' and i == n - 1:
            ea = True
            i += 1
            continue
        if c in '*+?{':
            if not items or last_rep:
                raise Bad()
            if c == '*': mn, mx = 0, None; i += 1
            elif c == '+': mn, mx = 1, None; i += 1
            elif c == '?': mn, mx = 0, 1; i += 1
            else:
                j = p.find('}', i)
                if j < 0: raise Bad()
                body = p[i+1:j]
                parts = body.split(',')
                if len(parts) == 1 and parts[0].isdigit() and parts[0].isascii():
                    mn = mx = int(parts[0])
                elif len(parts) == 2 and all(x.isdigit() and x.isascii() for x in parts):
                    mn, mx = int(parts[0]), int(parts[1])
                    if mn > mx: raise Bad()
                else:
                    raise Bad()
                i = j + 1
            items[-1][1] = mn
            items[-1][2] = mx
            last_rep = True
            continue
        last_rep = False
        if c == '\\':
            if i + 1 >= n: raise Bad()
            items.append([('c', p[i+1]), 1, 1])
            i += 2
        elif c == '.':
            items.append([('any',), 1, 1]); i += 1
        elif c == '[':
            j = i + 1
            neg = False
            if j < n and p[j] == '^':
                neg = True; j += 1
            chars = []
            while j < n and p[j] != ']':
                chars.append(p[j]); j += 1
            if j >= n: raise Bad()
            if not chars: raise Bad()
            singles = set(); ranges = []
            k = 0
            while k < len(chars):
                if k + 2 < len(chars) and chars[k+1] == '-':
                    if chars[k] > chars[k+2]: raise Bad()
                    ranges.append((chars[k], chars[k+2])); k += 3
                else:
                    singles.add(chars[k]); k += 1
            items.append([('set', neg, singles, ranges), 1, 1])
            i = j + 1
        else:
            items.append([('c', c), 1, 1]); i += 1
    return sa, ea, items

def m1(m, ch):
    t = m[0]
    if t == 'c': return ch == m[1]
    if t == 'any': return True
    _, neg, s, rs = m
    r = ch in s or any(a <= ch <= b for a, b in rs)
    return r != neg

def ends(items, line, start):
    cur = {start}
    L = len(line)
    for m, mn, mx in items:
        nxt = set()
        for s in cur:
            pos = s
            cnt = 0
            if mn == 0: nxt.add(pos)
            while (mx is None or cnt < mx) and pos < L and m1(m, line[pos]):
                pos += 1; cnt += 1
                if cnt >= mn: nxt.add(pos)
        cur = nxt
        if not cur: break
    return cur

def main():
    data = sys.stdin.read().split('\n')
    if data and data[-1] == '':
        data.pop()
    if not data:
        print('bad pattern'); return
    try:
        sa, ea, items = parse(data[0])
    except Bad:
        print('bad pattern'); return
    lines = data[1:]
    k = 0
    out = []
    for ln, line in enumerate(lines, 1):
        L = len(line)
        found = None
        for s in range(0, (1 if sa else L + 1)):
            e = ends(items, line, s)
            if ea: e = {x for x in e if x == L}
            if e:
                found = (s, max(e)); break
        if found:
            k += 1
            out.append("%d: %d-%d [%s]" % (ln, found[0], found[1], line[found[0]:found[1]]))
    out.append("%d of %d lines match" % (k, len(lines)))
    print('\n'.join(out))
main()
