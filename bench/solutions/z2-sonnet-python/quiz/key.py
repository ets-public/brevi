import re
from decimal import Decimal

DEC = re.compile(r'^[+-]?(\d+\.?\d*|\.\d+)$')


def parse_key_line(line, seen):
    """Return (id, question dict) or None if bad."""
    w = line.split()
    if len(w) < 4:
        return None
    qid, typ, pts = w[0], w[1], w[2]
    if not re.match(r'^Q\d+$', qid) or qid in seen:
        return None
    if not re.match(r'^\d+$', pts) or not 1 <= int(pts) <= 100:
        return None
    pts = int(pts)
    spec = w[3:]
    if typ == 'choice' and len(spec) == 1 and re.match(r'^[A-E]$', spec[0]):
        return qid, {'type': typ, 'points': pts, 'spec': spec[0]}
    if typ == 'multi' and len(spec) == 1 and re.match(r'^[A-E]{2,}$', spec[0]) \
            and len(set(spec[0])) == len(spec[0]):
        return qid, {'type': typ, 'points': pts, 'spec': set(spec[0])}
    if typ == 'number' and len(spec) == 2 and DEC.match(spec[0]) and DEC.match(spec[1]):
        tol = Decimal(spec[1])
        if tol < 0:
            return None
        return qid, {'type': typ, 'points': pts, 'spec': (Decimal(spec[0]), tol)}
    if typ == 'text' and len(spec) == 1:
        words = spec[0].split('|')
        if all(words):
            return qid, {'type': typ, 'points': pts, 'spec': set(x.lower() for x in words)}
    return None


def grade_answer(q, ans):
    pts, typ, spec = q['points'], q['type'], q['spec']
    if typ == 'choice':
        return pts if ans == spec else 0
    if typ == 'multi':
        s = set(ans)
        if not s <= set('ABCDE'):
            return 0
        if s == spec:
            return pts
        right = len(s & spec)
        wrong = len(s - spec)
        return max(0, pts * (right - wrong) // len(spec))
    if typ == 'number':
        if not DEC.match(ans):
            return 0
        return pts if abs(Decimal(ans) - spec[0]) <= spec[1] else 0
    return pts if ans.lower() in spec else 0
