from collections import deque

n = int(input())
walls = set()
for x in range(n):
    for y in range(n):
        if (x * 7 + y * 13) % 11 == 0 and (x + y) % 3 != 0:
            walls.add((x, y))
seen = {(0, 0)}
q = deque([(0, 0, 0)])
far = 0
while q:
    x, y, d = q.popleft()
    if d > far:
        far = d
    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        nx, ny = x + dx, y + dy
        if 0 <= nx < n and 0 <= ny < n and (nx, ny) not in walls and (nx, ny) not in seen:
            seen.add((nx, ny))
            q.append((nx, ny, d + 1))
print(len(seen), far)
