# Within every limit: runs as it would without them.
import sys
total = 0
for i in range(1000000):
    total += i * i
print(total)
print("to stderr", file=sys.stderr)
