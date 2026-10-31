import sys


def main():
    data = sys.stdin.read().split()
    if len(data) < 2:
        return
    r, c = int(data[0]), int(data[1])
    vals = [int(x) for x in data[2:2 + r * c]]
    m = [vals[i * c:(i + 1) * c] for i in range(r)]

    spiral = []
    top, bottom, left, right = 0, r - 1, 0, c - 1
    while top <= bottom and left <= right:
        for j in range(left, right + 1):
            spiral.append(m[top][j])
        top += 1
        for i in range(top, bottom + 1):
            spiral.append(m[i][right])
        right -= 1
        if top <= bottom:
            for j in range(right, left - 1, -1):
                spiral.append(m[bottom][j])
            bottom -= 1
        if left <= right:
            for i in range(bottom, top - 1, -1):
                spiral.append(m[i][left])
            left += 1

    sums = [sum(m[i][j] for i in range(r)) for j in range(c)]
    sys.stdout.write(" ".join(map(str, spiral)) + "\n")
    sys.stdout.write(" ".join(map(str, sums)) + "\n")


main()
