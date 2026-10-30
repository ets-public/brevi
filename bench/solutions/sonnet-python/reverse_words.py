import sys
for l in sys.stdin:
    print(" ".join(reversed(l.split())))
