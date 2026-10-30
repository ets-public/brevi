import sys

tokens = sys.stdin.read().split()
R, C = int(tokens[0]), int(tokens[1])
vals = list(map(int, tokens[2:2 + R * C]))
m = [vals[i * C:(i + 1) * C] for i in range(R)]

order = []
top, bottom, left, right = 0, R - 1, 0, C - 1
while top <= bottom and left <= right:
    for j in range(left, right + 1):
        order.append(m[top][j])
    for i in range(top + 1, bottom + 1):
        order.append(m[i][right])
    if top < bottom and left < right:
        for j in range(right - 1, left - 1, -1):
            order.append(m[bottom][j])
        for i in range(bottom - 1, top, -1):
            order.append(m[i][left])
    top += 1
    bottom -= 1
    left += 1
    right -= 1

sums = [sum(m[i][j] for i in range(R)) for j in range(C)]
print(' '.join(map(str, order)))
print(' '.join(map(str, sums)))
