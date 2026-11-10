# An int outgrows 64 bits after the program has read stdin and printed: the dynamic build's second run gets the
# same stdin, and only output the first run had not written yet is passed on.
import sys

n = int(input())
print("start")
xs = [1]
for i in range(1, n + 1):
    xs[0] = xs[0] * i
    if i % 10 == 0:
        print(i, xs[0] % 1000003)
print(xs[0])
print("rest", sys.stdin.read().split())
