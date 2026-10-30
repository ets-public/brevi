import sys

total = 0
for line in sys.stdin:
    s = line.strip()
    if s:
        total += int(s)
print(total)
