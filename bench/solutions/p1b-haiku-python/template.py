import json
import re
import sys

MISSING = object()
TAG = re.compile(r"\{\{(.*?)\}\}")


def to_text(v):
    if v is MISSING or v is None:
        return ""
    if isinstance(v, str):
        return v
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        return json.dumps(v)
    return json.dumps(v, separators=(",", ":"))


def truthy(v):
    if v is MISSING or v is None or v is False:
        return False
    if isinstance(v, bool):
        return True
    if isinstance(v, (int, float)):
        return v != 0
    if isinstance(v, (str, list)):
        return len(v) > 0
    return True


def lookup(name, stack):
    if name == ".":
        for frame in reversed(stack):
            if frame[0] == "loop":
                return frame[1]
        return stack[0][1]
    if name == "@index":
        for frame in reversed(stack):
            if frame[0] == "loop":
                return frame[2]
        return MISSING
    parts = name.split(".")
    head = parts[0]
    for frame in reversed(stack):
        obj = frame[1]
        if isinstance(obj, dict) and head in obj:
            v = obj[head]
            for p in parts[1:]:
                if isinstance(v, dict) and p in v:
                    v = v[p]
                else:
                    return MISSING
            return v
    return MISSING


def apply_filter(v, f):
    if f == "upper":
        return to_text(v).upper()
    if f == "lower":
        return to_text(v).lower()
    if f == "len":
        if isinstance(v, (str, list, dict)):
            return len(v)
        return MISSING
    if f.startswith("default:"):
        if v is MISSING or v is None or v == "":
            return f[len("default:"):]
        return v
    return v


def parse_nodes(items, i):
    nodes = []
    while i < len(items):
        kind, val = items[i]
        if kind == "text":
            nodes.append(("text", val))
            i += 1
            continue
        c = val.strip()
        if c in ("/each", "/if", "else"):
            return nodes, i + 1, c
        if c.startswith("#each "):
            name = c[len("#each "):].strip()
            body, i, _ = parse_nodes(items, i + 1)
            nodes.append(("each", name, body))
        elif c.startswith("#if "):
            name = c[len("#if "):].strip()
            then, i, term = parse_nodes(items, i + 1)
            els = []
            if term == "else":
                els, i, term = parse_nodes(items, i)
            nodes.append(("if", name, then, els))
        else:
            nodes.append(("var", c))
            i += 1
    return nodes, i, None


def render(nodes, stack):
    out = []
    for node in nodes:
        kind = node[0]
        if kind == "text":
            out.append(node[1])
        elif kind == "var":
            parts = [p.strip() for p in node[1].split("|")]
            v = lookup(parts[0], stack)
            for f in parts[1:]:
                v = apply_filter(v, f)
            out.append(to_text(v))
        elif kind == "each":
            arr = lookup(node[1], stack)
            if isinstance(arr, list):
                for idx, elem in enumerate(arr):
                    stack.append(("loop", elem, idx))
                    out.append(render(node[2], stack))
                    stack.pop()
        elif kind == "if":
            if truthy(lookup(node[1], stack)):
                out.append(render(node[2], stack))
            else:
                out.append(render(node[3], stack))
    return "".join(out)


def main():
    raw = sys.stdin.read()
    first, _, template = raw.partition("\n")
    data = json.loads(first)

    pieces = TAG.split(template)
    items = []
    for idx, part in enumerate(pieces):
        if idx % 2 == 0:
            if part:
                items.append(("text", part))
        else:
            items.append(("tag", part))

    nodes, _, _ = parse_nodes(items, 0)
    sys.stdout.write(render(nodes, [("root", data)]))


if __name__ == "__main__":
    main()
