import sys
sys.setrecursionlimit(100000)
def depth(n):
    return 0 if n == 0 else 1 + depth(n - 1)
n = int(input())
print(depth(n))
