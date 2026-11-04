d = {1: 1, 2: 2}
e = {1: 5, 2: 6, 3: 7}
print(d.keys() < e.keys(), d.keys() <= e.keys(), e.keys() > d.keys(), d.keys() == {1, 2}, d.keys() == e.keys())
print({1: 1}.keys() == {1: 2}.keys(), d.items() <= e.items(), d.items() == {(1, 1), (2, 2)}, d.values() == d.values())
print(d.keys() != {1}, {1, 2} == d.keys(), d.keys() >= {1})
print(sorted(d.keys() | [3]), sorted(d.keys() & {2, 9}))
