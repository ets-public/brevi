# Unbounded recursion raises RecursionError (caught here), and a deep but bounded one runs after
# sys.setrecursionlimit, as in CPython.
import sys
def down(n):
    return down(n + 1)
try:
    down(0)
except RecursionError as e:
    print("RecursionError", type(e).__name__)
sys.setrecursionlimit(20000)
def depth(n):
    return 0 if n == 0 else 1 + depth(n - 1)
print(depth(15000))
