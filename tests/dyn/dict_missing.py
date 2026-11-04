class Counts(dict):
    def __missing__(self, key):
        self[key] = 0
        return 0
c = Counts()
c["a"] += 2
c["b"]
print(sorted(c.items()), "z" in c, c.get("z"))
