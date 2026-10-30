import sys

PAIRS = {')': '(', ']': '[', '}': '{'}
OPEN = set(PAIRS.values())

for raw in sys.stdin:
    line = raw.rstrip('\r\n')
    stack = []
    result = None
    for pos, ch in enumerate(line, start=1):
        if ch in OPEN:
            stack.append(ch)
        elif ch in PAIRS:
            if not stack or stack[-1] != PAIRS[ch]:
                result = f"error at {pos}"
                break
            stack.pop()
    if result is None:
        result = "ok" if not stack else f"error at {len(line) + 1}"
    print(result)
