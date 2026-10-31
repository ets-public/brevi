import sys

pairs = {")": "(", "]": "[", "}": "{"}

for raw in sys.stdin:
    line = raw.rstrip("\r\n")
    stack = []
    result = None
    for i, c in enumerate(line, 1):
        if c in "([{":
            stack.append(c)
        elif c in pairs:
            if not stack or stack[-1] != pairs[c]:
                result = "error at %d" % i
                break
            stack.pop()
    if result is None:
        result = "ok" if not stack else "error at %d" % (len(line) + 1)
    print(result)
