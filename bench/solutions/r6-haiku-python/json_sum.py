import sys
import json


def total(node):
    if isinstance(node, bool) or node is None:
        return 0
    if isinstance(node, int):
        return node
    if isinstance(node, float):
        return 0
    if isinstance(node, str):
        return 0
    if isinstance(node, list):
        return sum(total(x) for x in node)
    if isinstance(node, dict):
        if any(isinstance(v, str) and v == "red" for v in node.values()):
            return 0
        return sum(total(v) for v in node.values())
    return 0


def main():
    data = json.loads(sys.stdin.read())
    print(total(data))


main()
