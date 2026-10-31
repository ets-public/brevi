import sys


class Bad(Exception):
    pass


def isnum(s):
    return s != '' and all(ch in '0123456789' for ch in s)


def parse_set(p, i):
    # p[i] == '['
    j = i + 1
    neg = False
    if j < len(p) and p[j] == '^':
        neg = True
        j += 1
    close = p.find(']', j)
    if close < 0:
        raise Bad()
    content = p[j:close]
    if content == '':
        raise Bad()
    singles = set()
    ranges = []
    k = 0
    while k < len(content):
        if k + 2 < len(content) and content[k + 1] == '-':
            lo, hi = content[k], content[k + 2]
            if lo > hi:
                raise Bad()
            ranges.append((lo, hi))
            k += 3
        else:
            singles.add(content[k])
            k += 1

    def matcher(c, singles=singles, ranges=ranges, neg=neg):
        inside = c in singles or any(lo <= c <= hi for lo, hi in ranges)
        return inside != neg

    return matcher, close + 1


def parse_brace(p, i):
    # p[i] == '{'
    close = p.find('}', i)
    if close < 0:
        raise Bad()
    body = p[i + 1:close]
    parts = body.split(',')
    if len(parts) == 1:
        if not isnum(body):
            raise Bad()
        lo = hi = int(body)
    elif len(parts) == 2:
        a, b = parts
        if not (isnum(a) and isnum(b)):
            raise Bad()
        lo, hi = int(a), int(b)
        if lo > hi:
            raise Bad()
    else:
        raise Bad()
    return lo, hi, close + 1


def parse(p):
    items = []  # each: [matcher, lo, hi]
    anchor_s = False
    anchor_e = False
    n = len(p)
    i = 0
    last_rep = False
    while i < n:
        c = p[i]
        if c in '*+?{':
            if not items or last_rep:
                raise Bad()
            if c == '*':
                lo, hi, i = 0, None, i + 1
            elif c == '+':
                lo, hi, i = 1, None, i + 1
            elif c == '?':
                lo, hi, i = 0, 1, i + 1
            else:
                lo, hi, i = parse_brace(p, i)
            items[-1][1] = lo
            items[-1][2] = hi
            last_rep = True
            continue
        last_rep = False
        if c == '^' and i == 0:
            anchor_s = True
            i += 1
            continue
        if c == '$' and i == n - 1:
            anchor_e = True
            i += 1
            continue
        if c == '[':
            m, i = parse_set(p, i)
        elif c == '.':
            m = (lambda ch: True)
            i += 1
        elif c == '\\':
            if i + 1 >= n:
                raise Bad()
            lit = p[i + 1]
            m = (lambda ch, lit=lit: ch == lit)
            i += 2
        else:
            m = (lambda ch, lit=c: ch == lit)
            i += 1
        items.append([m, 1, 1])
    return anchor_s, items, anchor_e


def match_at(items, anchor_e, text, start):
    """Return the longest end position of a match starting at start, or None."""
    n = len(text)
    positions = {start}
    for m, lo, hi in items:
        result = set()
        cur = positions
        if lo == 0:
            result |= cur
        k = 0
        while cur:
            if hi is not None and k >= hi:
                break
            nxt = set()
            for q in cur:
                if q < n and m(text[q]):
                    nxt.add(q + 1)
            k += 1
            cur = nxt
            if k >= lo:
                result |= cur
        positions = result
        if not positions:
            return None
    if anchor_e:
        positions = {q for q in positions if q == n}
    if not positions:
        return None
    return max(positions)


def main():
    data = sys.stdin.read()
    lines = data.split('\n')
    if data.endswith('\n'):
        lines.pop()
    lines = [l[:-1] if l.endswith('\r') else l for l in lines]
    pat = lines[0] if lines else ''
    texts = lines[1:]
    try:
        anchor_s, items, anchor_e = parse(pat)
    except Bad:
        print("bad pattern")
        return
    out = []
    k = 0
    for idx, t in enumerate(texts, 1):
        starts = [0] if anchor_s else range(len(t) + 1)
        found = None
        for s in starts:
            e = match_at(items, anchor_e, t, s)
            if e is not None:
                found = (s, e)
                break
        if found is not None:
            k += 1
            s, e = found
            out.append(f"{idx}: {s}-{e} [{t[s:e]}]")
    out.append(f"{k} of {len(texts)} lines match")
    print("\n".join(out))


if __name__ == "__main__":
    main()
