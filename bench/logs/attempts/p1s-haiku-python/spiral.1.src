import sys

data = sys.stdin.read().split()
R, C = int(data[0]), int(data[1])
vals = list(map(int, data[2:2 + R * C]))
M = [vals[r * C:(r + 1) * C] for r in range(R)]

out = []
top, bottom, left, right = 0, R - 1, 0, C - 1
while top <= bottom and left <= right:
    for c in range(left, right + 1):
        out.append(M[top][c])
    top += 1
    for r in range(top, bottom + 1):
        out.append(M[r][right])
    right -= 1
    if top <= bottom:
        for c in range(right, left - 1, -1):
            out.append(M[bottom][c])
        bottom -= 1
    if left <= right:
        for r in range(bottom, top - 1, -1):
            out.append(M[r][left])
        left += 1

sums = [sum(M[r][c] for r in range(R)) for c in range(C)]
print(" ".join(map(str, out)))
print(" ".join(map(str, sums)))
