KIND = {'car': 'C', 'moto': 'M', 'van': 'V'}


class Garage:
    def __init__(self, cars, motos, vans):
        self.spaces = {
            'C': [None] * cars,
            'M': [None] * motos,
            'V': [None] * vans,
        }
        self.inside = {}  # plate -> (vtype, space_name)

    def _free(self, kind):
        slots = self.spaces[kind]
        for i, p in enumerate(slots):
            if p is None:
                return i
        return None

    def allocate(self, vtype, plate):
        """Return the space name taken, or None if full."""
        if vtype == 'car':
            kinds = ['C']
        elif vtype == 'van':
            kinds = ['V']
        else:
            kinds = ['M', 'C']
        for kind in kinds:
            i = self._free(kind)
            if i is not None:
                self.spaces[kind][i] = plate
                name = '%s%d' % (kind, i + 1)
                self.inside[plate] = (vtype, name)
                return name
        return None

    def release(self, plate):
        vtype, name = self.inside.pop(plate)
        kind = name[0]
        self.spaces[kind][int(name[1:]) - 1] = None
        return vtype

    def occupied(self, kind):
        return sum(1 for p in self.spaces[kind] if p is not None)

    def total(self, kind):
        return len(self.spaces[kind])
