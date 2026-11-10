def keep(xs, store):
    store.append(xs)
store = []
row = [1]
keep(row, store)
row.append(2)
print(store)
