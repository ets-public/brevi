class Garage:
    def __init__(self, car, moto, van):
        self.free = {'C': [True] * car, 'M': [True] * moto, 'V': [True] * van}
        self.inside = {}  # plate -> (type, space, time)

    def _take(self, letter):
        for i, f in enumerate(self.free[letter]):
            if f:
                self.free[letter][i] = False
                return '%s%d' % (letter, i + 1)
        return None

    def enter(self, plate, kind, when):
        if kind == 'car':
            sp = self._take('C')
        elif kind == 'van':
            sp = self._take('V')
        else:
            sp = self._take('M') or self._take('C')
        if sp is None:
            return None
        self.inside[plate] = (kind, sp, when)
        return sp

    def leave(self, plate):
        kind, sp, when = self.inside.pop(plate)
        self.free[sp[0]][int(sp[1:]) - 1] = True
        return kind, when

    def counts(self):
        c = {'C': 0, 'M': 0, 'V': 0}
        for _, sp, _ in self.inside.values():
            c[sp[0]] += 1
        return c
