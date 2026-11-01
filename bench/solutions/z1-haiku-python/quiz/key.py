import re
from decimal import Decimal

ID_RE = re.compile(r"^Q\d+$")
POINTS_RE = re.compile(r"^\d+$")
CHOICE_RE = re.compile(r"^[A-E]$")
MULTI_RE = re.compile(r"^[A-E]{2,}$")
NUM_RE = re.compile(r"^[+-]?(\d+\.?\d*|\.\d+)$")
LETTERS = "ABCDE"


def parse_key_line(text):
    """Return a question dict for a valid key line, otherwise None."""
    parts = text.split()
    if len(parts) < 3:
        return None
    qid, typ, pts = parts[0], parts[1], parts[2]
    if not ID_RE.match(qid):
        return None
    if not POINTS_RE.match(pts):
        return None
    points = int(pts)
    if not 1 <= points <= 100:
        return None

    data = None
    if typ == "choice" and len(parts) == 4 and CHOICE_RE.match(parts[3]):
        data = parts[3]
    elif typ == "multi" and len(parts) == 4 and MULTI_RE.match(parts[3]):
        if len(set(parts[3])) != len(parts[3]):
            return None
        data = set(parts[3])
    elif typ == "number" and len(parts) == 5:
        if not (NUM_RE.match(parts[3]) and NUM_RE.match(parts[4])):
            return None
        tol = Decimal(parts[4])
        if tol < 0:
            return None
        data = (Decimal(parts[3]), tol)
    elif typ == "text" and len(parts) == 4:
        words = parts[3].split("|")
        if not all(words):
            return None
        data = words
    else:
        return None

    return {"id": qid, "type": typ, "points": points, "data": data}


def parse_key(key_lines):
    """key_lines: list of (lineno, text). Returns (questions, messages)."""
    questions = []
    messages = []
    seen = set()
    for lineno, text in key_lines:
        q = parse_key_line(text)
        if q is None or q["id"] in seen:
            messages.append("key line %d: bad question" % lineno)
            continue
        seen.add(q["id"])
        questions.append(q)
    return questions, messages


def grade_answer(q, ans):
    """Score one non-empty answer string against question q."""
    t = q["type"]
    points = q["points"]
    if t == "choice":
        return points if ans == q["data"] else 0
    if t == "multi":
        if any(c not in LETTERS for c in ans):
            return 0
        letters = set(ans)
        key = q["data"]
        if letters == key:
            return points
        right = len(letters & key)
        wrong = len(letters - key)
        return max(0, points * (right - wrong) // len(key))
    if t == "number":
        if not NUM_RE.match(ans):
            return 0
        value, tol = q["data"]
        return points if abs(Decimal(ans) - value) <= tol else 0
    if t == "text":
        lowered = [w.lower() for w in q["data"]]
        return points if ans.lower() in lowered else 0
    return 0
