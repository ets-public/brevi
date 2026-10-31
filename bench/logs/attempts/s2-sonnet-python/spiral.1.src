import sys
t = sys.stdin.read().split()
R, C = int(t[0]), int(t[1])
m = [[int(t[2 + i * C + j]) for j in range(C)] for i in range(R)]
out = []
top, bot, l, r = 0, R - 1, 0, C - 1
while top <= bot and l <= r:
    for j in range(l, r + 1): out.append(m[top][j])
    top += 1
    for i in range(top, bot + 1): out.append(m[i][r])
    r -= 1
    if top <= bot:
        for j in range(r, l - 1, -1): out.append(m[bot][j])
        bot -= 1
    if l <= r:
        for i in range(bot, top - 1, -1): out.append(m[i][l])
        l += 1
print(' '.join(map(str, out)))
print(' '.join(str(sum(m[i][j] for i in range(R))) for j in range(C)))
