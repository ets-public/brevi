import sys

data = sys.stdin.read().split()
R, C = int(data[0]), int(data[1])
vals = [int(x) for x in data[2:2 + R * C]]
M = [vals[i * C:(i + 1) * C] for i in range(R)]

order = []
top, bottom, left, right = 0, R - 1, 0, C - 1
while top <= bottom and left <= right:
    for c in range(left, right + 1):
        order.append(M[top][c])
    top += 1
    for r in range(top, bottom + 1):
        order.append(M[r][right])
    right -= 1
    if top <= bottom:
        for c in range(right, left - 1, -1):
            order.append(M[bottom][c])
        bottom -= 1
    if left <= right:
        for r in range(bottom, top - 1, -1):
            order.append(M[r][left])
        left += 1

print(" ".join(str(x) for x in order))
sums = [sum(M[r][c] for r in range(R)) for c in range(C)]
print(" ".join(str(x) for x in sums))
