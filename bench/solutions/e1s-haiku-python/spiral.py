import sys

tokens = sys.stdin.read().split()
R = int(tokens[0])
C = int(tokens[1])
vals = list(map(int, tokens[2:2 + R * C]))
M = [vals[i * C:(i + 1) * C] for i in range(R)]

res = []
top, bottom, left, right = 0, R - 1, 0, C - 1
while top <= bottom and left <= right:
    for j in range(left, right + 1):
        res.append(M[top][j])
    top += 1
    for i in range(top, bottom + 1):
        res.append(M[i][right])
    right -= 1
    if top <= bottom:
        for j in range(right, left - 1, -1):
            res.append(M[bottom][j])
        bottom -= 1
    if left <= right and top <= bottom:
        for i in range(bottom, top - 1, -1):
            res.append(M[i][left])
        left += 1

sums = [sum(M[i][j] for i in range(R)) for j in range(C)]

print(" ".join(map(str, res)))
print(" ".join(map(str, sums)))
