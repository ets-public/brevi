import sys

pairs = {')': '(', ']': '[', '}': '{'}

for raw in sys.stdin:
    line = raw.rstrip('\r\n')
    stack = []
    result = None
    for i, c in enumerate(line, 1):
        if c in '([{':
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
