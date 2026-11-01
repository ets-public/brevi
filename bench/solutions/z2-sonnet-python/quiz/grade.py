class Gradebook:
    def __init__(self, qids, maxpts):
        self.qids = qids
        self.maxpts = maxpts
        self.names = []
        self.scores = {}  # name -> {qid: score}

    def has(self, name):
        return name in self.scores

    def add(self, name, per_q):
        self.names.append(name)
        self.scores[name] = per_q

    def total(self, name):
        return sum(self.scores[name].values())

    def pct(self, name):
        return self.total(name) * 100 // self.maxpts

    def avg(self, qid):
        n = len(self.names)
        t = sum(self.scores[x].get(qid, 0) for x in self.names)
        v = (t * 200 + n) // (2 * n)
        return '%d.%02d' % (v // 100, v % 100)

    def top(self):
        best = max(self.total(x) for x in self.names)
        return [x for x in self.names if self.total(x) == best], best
