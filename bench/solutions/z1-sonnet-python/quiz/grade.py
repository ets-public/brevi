class Grades:
    def __init__(self, qids, maxpts):
        self.qids = qids
        self.maxpts = maxpts
        self.students = []
        self.names = set()

    def has(self, name):
        return name in self.names

    def add(self, name, scores):
        self.names.add(name)
        self.students.append((name, scores))

    def total(self, scores):
        return sum(scores.values())

    def pct(self, scores):
        return self.total(scores) * 100 // self.maxpts

    def avg(self, qid):
        n = len(self.students)
        tot = sum(s.get(qid, 0) for _, s in self.students)
        c = (tot * 200 + n) // (2 * n)
        return '%d.%02d' % (c // 100, c % 100)

    def top(self):
        best = max(self.total(s) for _, s in self.students)
        return [nm for nm, s in self.students if self.total(s) == best], best
