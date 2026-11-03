# min/max of three or more values, and built-ins passed to map() (both used to send programs to the slower dynamic build).
def edit_step(a, b, c):
    return 1 + min(a, b, c)


grid = [[1, 0, 1], [1, 1, 0]]
print(edit_step(3, 1, 2), max(4, 9, 2, 7), min(5.5, 2.5, 9.0))
print(sum(map(sum, grid)), list(map(len, grid)), list(map(abs, [-1, 2])), list(map(max, grid)), list(map(sorted, [[3, 1, 2]])))
