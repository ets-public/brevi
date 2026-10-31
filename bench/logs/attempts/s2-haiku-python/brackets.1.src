import sys

PAIRS = {")": "(", "]": "[", "}": "{"}
OPEN = set("([{")

for line in sys.stdin:
    line = line.rstrip("\n").rstrip("\r")
    stack = []
    err = None
    for i, c in enumerate(line, start=1):
        if c in OPEN:
            stack.append(c)
        elif c in PAIRS:
            if not stack or stack[-1] != PAIRS[c]:
                err = i
                break
            stack.pop()
    if err is None and stack:
        err = len(line) + 1
    print("ok" if err is None else "error at %d" % err)
