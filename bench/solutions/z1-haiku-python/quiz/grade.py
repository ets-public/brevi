from decimal import Decimal, ROUND_HALF_UP

from key import grade_answer


class Gradebook:
    def __init__(self, questions):
        self.questions = questions
        self.max_total = sum(q["points"] for q in questions)
        self.students = []  # list of (name, {qid: score}) in input order

    def add(self, name, answers):
        scores = {}
        for q in self.questions:
            ans = answers.get(q["id"])
            scores[q["id"]] = 0 if ans is None else grade_answer(q, ans)
        self.students.append((name, scores))

    def total(self, scores):
        return sum(scores.values())

    def results(self):
        """List of (name, score, percent) in input order."""
        out = []
        for name, scores in self.students:
            score = self.total(scores)
            pct = score * 100 // self.max_total if self.max_total else 0
            out.append((name, score, pct))
        return out

    def average(self, qid):
        n = len(self.students)
        total = sum(scores[qid] for _, scores in self.students)
        avg = (Decimal(total) / Decimal(n)).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)
        return "{:.2f}".format(avg)

    def top(self):
        """Return (best score, names with that score in input order)."""
        totals = [(name, self.total(scores)) for name, scores in self.students]
        best = max(t for _, t in totals)
        names = [name for name, t in totals if t == best]
        return best, names
