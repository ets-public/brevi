import re
from decimal import Decimal

LETTERS = 'ABCDE'
ID_RE = re.compile(r'Q[0-9]+')
PTS_RE = re.compile(r'[0-9]+')
MULTI_RE = re.compile(r'[A-E]{2,}')
DEC_RE = re.compile(r'[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)')


def is_decimal(s):
    return DEC_RE.fullmatch(s) is not None


def parse_question(tokens):
    if len(tokens) < 4:
        return None
    qid, typ, pts = tokens[0], tokens[1], tokens[2]
    if not ID_RE.fullmatch(qid) or not PTS_RE.fullmatch(pts):
        return None
    points = int(pts)
    if not 1 <= points <= 100:
        return None
    base = {'id': qid, 'points': points, 'type': typ}
    if typ == 'choice' and len(tokens) == 4 and tokens[3] in tuple(LETTERS):
        base['answer'] = tokens[3]
        return base
    if typ == 'multi' and len(tokens) == 4:
        letters = tokens[3]
        if MULTI_RE.fullmatch(letters) and len(set(letters)) == len(letters):
            base['letters'] = frozenset(letters)
            return base
        return None
    if typ == 'number' and len(tokens) == 5:
        value, tol = tokens[3], tokens[4]
        if is_decimal(value) and is_decimal(tol) and not tol.startswith('-'):
            base['value'] = Decimal(value)
            base['tol'] = Decimal(tol)
            return base
        return None
    if typ == 'text' and len(tokens) == 4:
        words = tokens[3].split('|')
        if all(words):
            base['words'] = set(w.lower() for w in words)
            return base
    return None


def add_question(questions, tokens):
    """Adds a key line to questions (dict id -> question). Returns False if bad."""
    if not tokens:
        return False
    q = parse_question(tokens)
    if q is None or q['id'] in questions:
        return False
    questions[q['id']] = q
    return True


def grade(q, answer):
    """Points earned for one answer string to question q."""
    pts = q['points']
    typ = q['type']
    if typ == 'choice':
        return pts if answer == q['answer'] else 0
    if typ == 'multi':
        if any(c not in LETTERS for c in answer):
            return 0
        chosen = set(answer)
        letters = q['letters']
        if chosen == letters:
            return pts
        right = len(chosen & letters)
        wrong = len(chosen - letters)
        return max(0, pts * (right - wrong) // len(letters))
    if typ == 'number':
        if not is_decimal(answer):
            return 0
        return pts if abs(Decimal(answer) - q['value']) <= q['tol'] else 0
    if typ == 'text':
        return pts if answer.lower() in q['words'] else 0
    return 0
