import sys
t = sys.stdin.read().split()
r, c = int(t[0]), int(t[1])
m = [[int(t[2 + i * c + j]) for j in range(c)] for i in range(r)]
top, bot, l, rt = 0, r - 1, 0, c - 1
out = []
while top <= bot and l <= rt:
    for j in range(l, rt + 1):
        out.append(m[top][j])
    for i in range(top + 1, bot + 1):
        out.append(m[i][rt])
    if top < bot:
        for j in range(rt - 1, l - 1, -1):
            out.append(m[bot][j])
    if l < rt:
        for i in range(bot - 1, top, -1):
            out.append(m[i][l])
    top += 1; bot -= 1; l += 1; rt -= 1
print(" ".join(map(str, out)))
print(" ".join(str(sum(m[i][j] for i in range(r))) for j in range(c)))
