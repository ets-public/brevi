import key


class Grade:
    def __init__(self, questions):
        self.questions = questions
        self.students = []  # (name, {qid: points}, total)
        self.names = set()

    def has_name(self, name):
        return name in self.names

    def add_student(self, name, answers):
        per = {}
        for qid, q in self.questions.items():
            if qid in answers:
                per[qid] = key.grade(q, answers[qid])
            else:
                per[qid] = 0
        total = sum(per.values())
        self.names.add(name)
        self.students.append((name, per, total))

    def max_total(self):
        return sum(q['points'] for q in self.questions.values())

    def student_lines(self):
        mx = self.max_total()
        lines = []
        for name, _, total in self.students:
            pct = total * 100 // mx
            lines.append('%s %d/%d %d%%' % (name, total, mx, pct))
        return lines

    def average_lines(self):
        n = len(self.students)
        lines = []
        for qid in self.questions:
            s = sum(per[qid] for _, per, _ in self.students)
            cents = (2 * s * 100 + n) // (2 * n)
            lines.append('%s avg %d.%02d' % (qid, cents // 100, cents % 100))
        return lines

    def top_line(self):
        best = max(total for _, _, total in self.students)
        names = [name for name, _, total in self.students if total == best]
        return 'top: %s (%d)' % (', '.join(names), best)
