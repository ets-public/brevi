# Reads all of stdin, then passes 64 bits: the re-run must see the same input.
import sys
s = 0
n = 0
for line in sys.stdin:
    s += int(line)
    n += 1
print(n)
x = s + 7
for _ in range(6):
    x = x * 1000003
print(x)
