import sys, re

lines = sys.stdin.read().split('\n')
heads = []
warns = []
fence = 0
for i, l in enumerate(lines, 1):
    if l.startswith('```'):
        fence = 0 if fence else i
        continue
    if fence:
        continue
    m = re.match(r'^(#{1,6}) (.*)$', l)
    if not m or not m.group(2).strip(' '):
        continue
    t = m.group(2).strip(' ')
    t = re.sub(r' +#+$', '', t) if re.search(r' #+$', t) else t
    t = t.strip(' ')
    heads.append((i, len(m.group(1)), t))
if fence:
    unclosed = 'line %d: unclosed code block' % fence
out = []
if heads:
    top = min(h[1] for h in heads)
    counters = {}
    used = {}
    pl = pd = None
    maxd = 0
    for ln, lv, t in heads:
        if pl is None:
            d = 1 + lv - top
        elif lv > pl + 1:
            warns.append('line %d: level jump' % ln)
            d = pd + 1
        else:
            d = 1 + lv - top
        pl, pd = lv, d
        maxd = max(maxd, d)
        counters[d] = counters.get(d, 0) + 1
        for k in list(counters):
            if k > d:
                counters[k] = 0
        num = '.'.join(str(counters.get(k, 0)) for k in range(1, d + 1))
        s = ''.join(c for c in t.lower() if ('a' <= c <= 'z') or ('0' <= c <= '9') or c in ' -')
        s = s.replace(' ', '-') or 'section'
        c = used.get(s, 0)
        used[s] = c + 1
        if c:
            s = '%s-%d' % (s, c)
        out.append('%s%s %s (#%s)' % ('  ' * (d - 1), num, t, s))
if fence:
    warns.append(unclosed)
for w in warns:
    print(w)
if heads:
    for o in out:
        print(o)
    print('%d headings, max depth %d' % (len(heads), maxd))
else:
    print('no headings')
