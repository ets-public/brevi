import sys

def main():
    tokens = sys.stdin.read().split()
    r, c = int(tokens[0]), int(tokens[1])
    vals = list(map(int, tokens[2:2 + r * c]))
    m = [vals[i * c:(i + 1) * c] for i in range(r)]
    res = []
    top, bottom, left, right = 0, r - 1, 0, c - 1
    while top <= bottom and left <= right:
        for j in range(left, right + 1):
            res.append(m[top][j])
        for i in range(top + 1, bottom + 1):
            res.append(m[i][right])
        if top < bottom and left < right:
            for j in range(right - 1, left - 1, -1):
                res.append(m[bottom][j])
            for i in range(bottom - 1, top, -1):
                res.append(m[i][left])
        top += 1
        bottom -= 1
        left += 1
        right -= 1
    sums = [sum(m[i][j] for i in range(r)) for j in range(c)]
    sys.stdout.write(' '.join(map(str, res)) + '\n')
    sys.stdout.write(' '.join(map(str, sums)) + '\n')

main()
