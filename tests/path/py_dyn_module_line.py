# A dynamic-build blocker inside an imported module is named with that module's file and line (#194).
import _clock


class P:
    def __init__(self, d):
        self.data = d

    def __getitem__(self, k):
        return self.data[k]

    def get(self, k, default=None):
        try:
            return self[k]
        except KeyError:
            return default


print(P({}).get("a"), _clock.stamp().tm_year)
