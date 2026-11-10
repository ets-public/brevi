def add(xs, v):
    xs.append(v)
grid = [[1], [2]]
add(grid[0], 5)
print(grid)
for r in grid:
    r.append(7)
print(grid)
r = grid[1]
r[0] -= 10
print(grid)
def total(rows):
    s = 0
    for row in rows:
        s += row[0]
    return s
print(total(grid))
