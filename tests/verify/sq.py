# Squares up to n (stdin); n = 7 ends with an uncaught exception, whose traceback frames verify leaves out.
import sys

n = int(sys.stdin.readline() or 3)
for i in range(n):
    print(i * i)
if n == 7:
    raise ValueError("seven")
