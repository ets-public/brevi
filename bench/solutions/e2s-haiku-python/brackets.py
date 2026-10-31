import sys

PAIRS = {")": "(", "]": "[", "}": "{"}


def check(line):
    stack = []
    for i, ch in enumerate(line, 1):
        if ch in "([{":
            stack.append(ch)
        elif ch in PAIRS:
            if not stack or stack[-1] != PAIRS[ch]:
                return f"error at {i}"
            stack.pop()
    if stack:
        return f"error at {len(line) + 1}"
    return "ok"


def main():
    out = []
    for line in sys.stdin.read().splitlines():
        out.append(check(line))
    if out:
        sys.stdout.write("\n".join(out) + "\n")


main()
