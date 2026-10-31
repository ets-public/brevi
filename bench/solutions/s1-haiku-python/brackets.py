import sys

pairs = {')': '(', ']': '[', '}': '{'}
openers = set(pairs.values())

for line in sys.stdin:
    line = line.rstrip("\r\n")
    stack = []
    result = None
    for i, c in enumerate(line, start=1):
        if c in openers:
            stack.append(c)
        elif c in pairs:
            if not stack or stack[-1] != pairs[c]:
                result = "error at %d" % i
                break
            stack.pop()
    if result is None:
        if stack:
            result = "error at %d" % (len(line) + 1)
        else:
            result = "ok"
    print(result)
