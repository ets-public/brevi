import sys

for line in sys.stdin:
    parts = line.split()
    if len(parts) < 2:
        continue
    print(int(parts[0]) + int(parts[1]))
