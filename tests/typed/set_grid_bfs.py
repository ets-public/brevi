from collections import deque

def bfs(w, h, walls):
    start = (0, 0)
    seen = {start}
    dist = {start: 0}
    q = deque([start])
    while q:
        x, y = q.popleft()
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            nx, ny = x + dx, y + dy
            if 0 <= nx < w and 0 <= ny < h and (nx, ny) not in walls and (nx, ny) not in seen:
                seen.add((nx, ny))
                dist[(nx, ny)] = dist[(x, y)] + 1
                q.append((nx, ny))
    return len(seen), max(dist.values())

def main():
    w, h = 400, 400
    walls = set()
    for i in range(0, w * h, 7):
        x, y = i % w, i // w
        if (x * 31 + y * 17) % 5 == 0 and (x, y) != (0, 0):
            walls.add((x, y))
    print(len(walls), bfs(w, h, walls))

main()
