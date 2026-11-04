# d = labels.setdefault(s, {}); lst = d.setdefault(k, []); lst changes: labels sees them.
def route():
    labels = {}
    def add(state, k, T, ids):
        d = labels.setdefault(state, {})
        lst = d.setdefault(k, [])
        for T2, i2 in lst:
            if T2 <= T and i2 <= ids:
                return False
        lst[:] = [(T2, i2) for (T2, i2) in lst if not (T <= T2 and ids <= i2)]
        lst.append((T, ids))
        return True
    add((0, 1), 1, 5, ("a", "x"))
    add((0, 1), 1, 3, ("a", "x"))
    add((0, 2), 1, 7, ("b", "y"))
    return labels
print(route())
