# xs.extend(j) / xs += j with j a JSON value walks it as Python does: a list's items, an object's keys.
import json

doc = json.loads('{"a": [1, 2, {"b": 3}], "c": {"d": 4}}')
stack = [doc]
seen = []
while stack:
    node = stack.pop()
    if isinstance(node, dict):
        stack.extend(node.values())
    elif isinstance(node, list):
        stack += node
    else:
        seen.append(str(node))
print(seen)
