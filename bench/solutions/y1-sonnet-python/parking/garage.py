class Garage:
    def __init__(self, car, moto, van):
        self.free = {
            'C': ['C%d' % i for i in range(1, car + 1)],
            'M': ['M%d' % i for i in range(1, moto + 1)],
            'V': ['V%d' % i for i in range(1, van + 1)],
        }
        self.cap = {'C': car, 'M': moto, 'V': van}
        self.inside = {}  # plate -> (type, space, arrival)

    def _take(self, letter):
        f = self.free[letter]
        if not f:
            return None
        return f.pop(0)

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
        f = self.free[sp[0]]
        f.append(sp)
        f.sort(key=lambda s: int(s[1:]))
        return kind, sp, when

    def count(self, letter):
        return sum(1 for _, sp, _ in self.inside.values() if sp[0] == letter)
