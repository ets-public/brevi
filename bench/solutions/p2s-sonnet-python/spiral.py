import sys
t = sys.stdin.read().split()
R, C = int(t[0]), int(t[1])
vals = list(map(int, t[2:2 + R * C]))
m = [vals[i * C:(i + 1) * C] for i in range(R)]
out = []
top, bot, left, right = 0, R - 1, 0, C - 1
while top <= bot and left <= right:
    for j in range(left, right + 1):
        out.append(m[top][j])
    top += 1
    for i in range(top, bot + 1):
        out.append(m[i][right])
    right -= 1
    if top <= bot:
        for j in range(right, left - 1, -1):
            out.append(m[bot][j])
        bot -= 1
    if left <= right:
        for i in range(bot, top - 1, -1):
            out.append(m[i][left])
        left += 1
print(' '.join(map(str, out)))
print(' '.join(str(sum(m[i][j] for i in range(R))) for j in range(C)))
