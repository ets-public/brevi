import re
import sys


def parse(pat):
    toks = []
    n = len(pat)
    i = 0
    if n and pat[0] == '^':
        toks.append(('bol',))
        i = 1
    while i < n:
        c = pat[i]
        if c == '\\':
            if i + 1 >= n:
                return None
            toks.append(('lit', pat[i + 1]))
            i += 2
            continue
        if c == '[':
            j = i + 1
            neg = False
            if j < n and pat[j] == '^':
                neg = True
                j += 1
            items = []
            closed = False
            while j < n:
                ch = pat[j]
                if ch == ']':
                    closed = True
                    break
                if ch == '-' and items and items[-1][0] == 'c' and j + 1 < n and pat[j + 1] != ']':
                    a = items[-1][1]
                    b = pat[j + 1]
                    if a > b:
                        return None
                    items[-1] = ('r', a, b)
                    j += 2
                    continue
                items.append(('c', ch))
                j += 1
            if not closed or not items:
                return None
            toks.append(('set', neg, items))
            i = j + 1
            continue
        if c in '*+?' or c == '{':
            if c == '{':
                m = re.match(r'\{([0-9]+)(?:,([0-9]+))?\}', pat[i:])
                if not m:
                    return None
                lo = int(m.group(1))
                hi = int(m.group(2)) if m.group(2) is not None else lo
                if hi < lo:
                    return None
                step = len(m.group(0))
            else:
                lo, hi = {'*': (0, None), '+': (1, None), '?': (0, 1)}[c]
                step = 1
            if not toks or toks[-1][0] in ('bol', 'rep'):
                return None
            toks.append(('rep', lo, hi))
            i += step
            continue
        if c == '.':
            toks.append(('any',))
        elif c == '$' and i == n - 1:
            toks.append(('eol',))
        else:
            toks.append(('lit', c))
        i += 1
    return toks


def atom_ok(tok, ch):
    k = tok[0]
    if k == 'lit':
        return ch == tok[1]
    if k == 'any':
        return True
    neg, items = tok[1], tok[2]
    hit = False
    for it in items:
        if it[0] == 'c':
            if ch == it[1]:
                hit = True
                break
        elif it[1] <= ch <= it[2]:
            hit = True
            break
    return hit != neg


def match_at(toks, s, p):
    S = {p}
    i = 0
    n = len(s)
    while i < len(toks):
        t = toks[i]
        if t[0] == 'bol':
            S = {q for q in S if q == 0}
            i += 1
        elif t[0] == 'eol':
            S = {q for q in S if q == n}
            i += 1
        else:
            lo, hi = 1, 1
            if i + 1 < len(toks) and toks[i + 1][0] == 'rep':
                lo, hi = toks[i + 1][1], toks[i + 1][2]
                i += 2
            else:
                i += 1
            new = set()
            reach = S
            k = 0
            while True:
                if k >= lo:
                    new |= reach
                if hi is not None and k >= hi:
                    break
                if not reach:
                    break
                nxt = set()
                for q in reach:
                    if q < n and atom_ok(t, s[q]):
                        nxt.add(q + 1)
                reach = nxt
                k += 1
            S = new
        if not S:
            return None
    return max(S)


def main():
    data = sys.stdin.read()
    if data == '':
        lines = []
    else:
        lines = data.split('\n')
        if data.endswith('\n'):
            lines = lines[:-1]
    lines = [l[:-1] if l.endswith('\r') else l for l in lines]
    pat = lines[0] if lines else ''
    texts = lines[1:]
    toks = parse(pat)
    if toks is None:
        print('bad pattern')
        return
    out = []
    k = 0
    for idx, text in enumerate(texts, 1):
        for p in range(len(text) + 1):
            e = match_at(toks, text, p)
            if e is not None:
                k += 1
                out.append('%d: %d-%d [%s]' % (idx, p, e, text[p:e]))
                break
    out.append('%d of %d lines match' % (k, len(texts)))
    print('\n'.join(out))


if __name__ == '__main__':
    main()
