import sys, re
lines = sys.stdin.read().split('\n')
warns = []
heads = []
infence = False
fence_line = 0
for i, l in enumerate(lines, 1):
    if l.startswith('```'):
        if infence:
            infence = False
        else:
            infence = True
            fence_line = i
        continue
    if infence:
        continue
    m = re.match(r'^(#{1,6}) (.*)$', l)
    if not m or m.group(2).strip() == '':
        continue
    t = m.group(2).strip()
    t = re.sub(r' +#+$', '', t).strip()
    heads.append((i, len(m.group(1)), t))
if infence:
    warns_end = ['line %d: unclosed code block' % fence_line]
else:
    warns_end = []
out = []
if heads:
    top = min(h[1] for h in heads)
    counters = [0] * 8
    counts = {}
    prevlevel = None
    prevdepth = 0
    maxd = 0
    for (ln, lv, t) in heads:
        if prevlevel is None:
            d = 1 + lv - top
        elif lv > prevlevel + 1:
            warns.append('line %d: level jump' % ln)
            d = prevdepth + 1
        else:
            d = 1 + lv - top
        prevlevel = lv
        prevdepth = d
        maxd = max(maxd, d)
        while len(counters) <= d:
            counters.append(0)
        counters[d] += 1
        for k in range(d + 1, len(counters)):
            counters[k] = 0
        num = '.'.join(str(counters[k]) for k in range(1, d + 1))
        s = re.sub(r'[^a-z0-9 -]', '', t.lower()).replace(' ', '-')
        if s == '':
            s = 'section'
        c = counts.get(s, 0)
        counts[s] = c + 1
        slug = s if c == 0 else '%s-%d' % (s, c)
        out.append('  ' * (d - 1) + '%s %s (#%s)' % (num, t, slug))
for w in warns + warns_end:
    print(w)
if heads:
    for o in out:
        print(o)
    print('%d headings, max depth %d' % (len(heads), maxd))
else:
    print('no headings')
