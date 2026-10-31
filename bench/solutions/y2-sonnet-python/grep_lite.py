import sys, re

def parse(p):
    n = len(p)
    i = 0
    anch_s = anch_e = False
    if p.startswith('^'):
        anch_s = True
        i = 1
    items = []
    while i < n:
        c = p[i]
        if c == '$' and i == n - 1:
            anch_e = True
            break
        if c in '*+?{':
            return None
        if c == '\\':
            if i + 1 >= n:
                return None
            ch = p[i + 1]
            pred = (lambda ch: lambda x: x == ch)(ch)
            i += 2
        elif c == '.':
            pred = lambda x: True
            i += 1
        elif c == '[':
            j = i + 1
            neg = False
            if j < n and p[j] == '^':
                neg = True
                j += 1
            if j >= n or p[j] == ']':
                return None
            singles = set()
            ranges = []
            while True:
                if j >= n:
                    return None
                if p[j] == ']':
                    break
                if j + 2 < n and p[j + 1] == '-' and p[j + 2] != ']':
                    if p[j] > p[j + 2]:
                        return None
                    ranges.append((p[j], p[j + 2]))
                    j += 3
                else:
                    singles.add(p[j])
                    j += 1
            i = j + 1
            pred = (lambda s, r, ng: lambda x: ((x in s) or any(a <= x <= b for a, b in r)) != ng)(singles, ranges, neg)
        else:
            pred = (lambda ch: lambda x: x == ch)(c)
            i += 1
        lo, hi = 1, 1
        if i < n:
            q = p[i]
            if q == '*':
                lo, hi = 0, None
                i += 1
            elif q == '+':
                lo, hi = 1, None
                i += 1
            elif q == '?':
                lo, hi = 0, 1
                i += 1
            elif q == '{':
                m = re.compile(r'\{(\d+)(?:,(\d+))?\}').match(p, i)
                if not m:
                    return None
                lo = int(m.group(1))
                hi = int(m.group(2)) if m.group(2) is not None else lo
                if lo > hi:
                    return None
                i = m.end()
        items.append((pred, lo, hi))
    return anch_s, anch_e, items

def match_at(items, anch_e, line, start):
    cur = {start}
    L = len(line)
    for pred, lo, hi in items:
        new = set()
        for p in cur:
            k = 0
            if lo == 0:
                new.add(p)
            while (hi is None or k < hi) and p + k < L and pred(line[p + k]):
                k += 1
                if k >= lo:
                    new.add(p + k)
        cur = new
        if not cur:
            return None
    if anch_e:
        cur = {e for e in cur if e == L}
    return max(cur) if cur else None

def main():
    lines = sys.stdin.read().split('\n')
    if lines and lines[-1] == '':
        lines.pop()
    if not lines:
        return
    res = parse(lines[0])
    if res is None:
        print('bad pattern')
        return
    anch_s, anch_e, items = res
    texts = lines[1:]
    k = 0
    for idx, line in enumerate(texts, 1):
        starts = [0] if anch_s else range(len(line) + 1)
        for s in starts:
            e = match_at(items, anch_e, line, s)
            if e is not None:
                print('%d: %d-%d [%s]' % (idx, s, e, line[s:e]))
                k += 1
                break
    print('%d of %d lines match' % (k, len(texts)))

main()
