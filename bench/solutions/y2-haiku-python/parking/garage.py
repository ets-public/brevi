class Garage:
    def __init__(self, cars, motos, vans):
        # kind letter -> list of spaces, each holding a plate or None
        self.spaces = {'C': [None] * cars, 'M': [None] * motos, 'V': [None] * vans}
        # plate -> (kind letter, index, vehicle type, entry time)
        self.where = {}

    def inside(self, plate):
        return plate in self.where

    def _lowest_free(self, kind):
        for i, plate in enumerate(self.spaces[kind]):
            if plate is None:
                return i
        return None

    def enter(self, plate, vtype, t):
        """Park the vehicle; return its space name, or None if there is no space."""
        if vtype == 'car':
            order = ['C']
        elif vtype == 'van':
            order = ['V']
        else:
            order = ['M', 'C']
        for kind in order:
            i = self._lowest_free(kind)
            if i is not None:
                self.spaces[kind][i] = plate
                self.where[plate] = (kind, i, vtype, t)
                return f"{kind}{i + 1}"
        return None

    def leave(self, plate):
        """Free the plate's space; return (vehicle type, entry time)."""
        kind, i, vtype, t = self.where.pop(plate)
        self.spaces[kind][i] = None
        return vtype, t

    def counts(self):
        """Kind letter -> (occupied, total)."""
        return {k: (sum(1 for p in v if p is not None), len(v))
                for k, v in self.spaces.items()}
