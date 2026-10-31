import sys

data = sys.stdin.read().split()
R, C = int(data[0]), int(data[1])
vals = list(map(int, data[2:2 + R * C]))
M = [vals[i * C:(i + 1) * C] for i in range(R)]

out = []
top, bottom, left, right = 0, R - 1, 0, C - 1
while top <= bottom and left <= right:
    for j in range(left, right + 1):
        out.append(M[top][j])
    top += 1
    for i in range(top, bottom + 1):
        out.append(M[i][right])
    right -= 1
    if top <= bottom:
        for j in range(right, left - 1, -1):
            out.append(M[bottom][j])
        bottom -= 1
    if left <= right:
        for i in range(bottom, top - 1, -1):
            out.append(M[i][left])
        left += 1

print(" ".join(map(str, out)))
sums = [sum(M[i][j] for i in range(R)) for j in range(C)]
print(" ".join(map(str, sums)))
