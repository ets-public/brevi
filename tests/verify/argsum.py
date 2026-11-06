import sys

total = sum(int(a) for a in sys.argv[1:])
print("sum", total)
print("done", file=sys.stderr)
