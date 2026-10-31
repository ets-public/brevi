import sys
t = sys.stdin.read().split()
r, c = int(t[0]), int(t[1])
m = [[int(t[2 + i * c + j]) for j in range(c)] for i in range(r)]
out = []
top, bot, l, rt = 0, r - 1, 0, c - 1
while top <= bot and l <= rt:
    for j in range(l, rt + 1):
        out.append(m[top][j])
    top += 1
    for i in range(top, bot + 1):
        out.append(m[i][rt])
    rt -= 1
    if top <= bot:
        for j in range(rt, l - 1, -1):
            out.append(m[bot][j])
        bot -= 1
    if l <= rt:
        for i in range(bot, top - 1, -1):
            out.append(m[i][l])
        l += 1
print(' '.join(map(str, out)))
print(' '.join(str(sum(m[i][j] for i in range(r))) for j in range(c)))
