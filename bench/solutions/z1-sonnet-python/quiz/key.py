import re
from fractions import Fraction
DEC = re.compile(r'^[+-]?\d+(\.\d+)?$')

def parse_key_line(line):
    w = line.split()
    if len(w) < 4 or not re.match(r'^Q\d+$', w[0]) or not re.match(r'^\d+$', w[2]):
        return None
    pts = int(w[2])
    if pts < 1 or pts > 100:
        return None
    t = w[1]
    if t == 'choice' and len(w) == 4 and re.match(r'^[A-E]$', w[3]):
        return (w[0], t, pts, w[3])
    if t == 'multi' and len(w) == 4 and re.match(r'^[A-E]{2,}$', w[3]) and len(set(w[3])) == len(w[3]):
        return (w[0], t, pts, frozenset(w[3]))
    if t == 'number' and len(w) == 5 and DEC.match(w[3]) and DEC.match(w[4]) and Fraction(w[4]) >= 0:
        return (w[0], t, pts, (Fraction(w[3]), Fraction(w[4])))
    if t == 'text' and len(w) == 4:
        ws = w[3].split('|')
        if all(ws):
            return (w[0], t, pts, [x.lower() for x in ws])
    return None

def grade_answer(q, ans):
    _, t, pts, spec = q
    if t == 'choice':
        return pts if ans == spec else 0
    if t == 'multi':
        if not re.match(r'^[A-E]+$', ans):
            return 0
        s = set(ans)
        if s == spec:
            return pts
        right = len(s & spec)
        wrong = len(s - spec)
        return max(0, pts * (right - wrong) // len(spec))
    if t == 'number':
        if not DEC.match(ans):
            return 0
        return pts if abs(Fraction(ans) - spec[0]) <= spec[1] else 0
    return pts if ans.lower() in spec else 0
