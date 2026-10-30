import sys
from collections import deque

grid = [line.rstrip("\r\n") for line in sys.stdin.read().splitlines()]
grid = [row for row in grid if row.strip()]

start = end = None
for r, row in enumerate(grid):
    for c, ch in enumerate(row):
        if ch == "S":
            start = (r, c)
        elif ch == "E":
            end = (r, c)

def open_cell(r, c):
    return 0 <= r < len(grid) and 0 <= c < len(grid[r]) and grid[r][c] != "#"

result = -1
if start and end:
    dist = {start: 0}
    q = deque([start])
    while q:
        r, c = q.popleft()
        if (r, c) == end:
            result = dist[(r, c)]
            break
        for dr, dc in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            nr, nc = r + dr, c + dc
            if open_cell(nr, nc) and (nr, nc) not in dist:
                dist[(nr, nc)] = dist[(r, c)] + 1
                q.append((nr, nc))
print(result)
