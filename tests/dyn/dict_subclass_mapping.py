class CustomDict(dict):
    pass
d = CustomDict([('a', 1), ('b', 2)])
print(list(dict(d).items()))
class R(dict):
    def keys(self):
        return reversed(list(dict.keys(self)))
    __iter__ = keys
r = R([('a', 1), ('b', 2)])
print(list(dict(r).items()))
class M:
    def __init__(self): self.d = {"x": 1, "y": 2}
    def keys(self): return self.d.keys()
    def __getitem__(self, k): return self.d[k]
print(dict(M()))
e = {}
e.update(M())
print(e)
