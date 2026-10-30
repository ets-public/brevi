import sys

pairs = {")": "(", "]": "[", "}": "{"}
for line in sys.stdin.read().splitlines():
    stack = []
    ok = True
    for ch in line:
        if ch in "([{":
            stack.append(ch)
        elif ch in pairs:
            if not stack or stack.pop() != pairs[ch]:
                ok = False
                break
    if ok and not stack:
        print("yes")
    else:
        print("no")
