import sys
import json


def main():
    data = sys.stdin.read()
    doc = json.loads(data)
    total = 0
    stack = [doc]
    while stack:
        node = stack.pop()
        if isinstance(node, dict):
            if any(isinstance(v, str) and v == "red" for v in node.values()):
                continue
            stack.extend(node.values())
        elif isinstance(node, list):
            stack.extend(node)
        elif isinstance(node, bool):
            continue
        elif isinstance(node, int):
            total += node
    print(total)


main()
