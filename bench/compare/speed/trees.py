class Node:
    def __init__(self, left, right):
        self.left = left
        self.right = right

def make(d):
    if d == 0:
        return Node(None, None)
    return Node(make(d - 1), make(d - 1))

def check(t):
    if t.left is None:
        return 1
    return 1 + check(t.left) + check(t.right)

n = int(input())
max_depth = max(6, n)
print(f"stretch tree of depth {max_depth + 1}\t check: {check(make(max_depth + 1))}")
long_lived = make(max_depth)
d = 4
while d <= max_depth:
    iters = 2 ** (max_depth - d + 4)
    total = 0
    for _ in range(iters):
        total += check(make(d))
    print(f"{iters}\t trees of depth {d}\t check: {total}")
    d += 2
print(f"long lived tree of depth {max_depth}\t check: {check(long_lived)}")
