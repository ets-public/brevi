import json
import re
import sys

MISSING = object()
TAG_RE = re.compile(r"\{\{(.*?)\}\}")


def parse(text):
    root = []
    frames = []  # (node, parent_list)
    cur = root
    pos = 0
    for m in TAG_RE.finditer(text):
        if m.start() > pos:
            cur.append(("text", text[pos:m.start()]))
        pos = m.end()
        tag = m.group(1).strip()
        if tag.startswith("#each "):
            node = ["each", tag[6:].strip(), []]
            cur.append(node)
            frames.append((node, cur))
            cur = node[2]
        elif tag.startswith("#if "):
            node = ["if", tag[4:].strip(), [], None]
            cur.append(node)
            frames.append((node, cur))
            cur = node[2]
        elif tag == "else":
            if frames and frames[-1][0][0] == "if":
                node = frames[-1][0]
                node[3] = []
                cur = node[3]
        elif tag in ("/each", "/if"):
            if frames:
                _, parent = frames.pop()
                cur = parent
        else:
            cur.append(("var", tag))
    if pos < len(text):
        cur.append(("text", text[pos:]))
    return root


def resolve_path(value, parts):
    for seg in parts:
        if isinstance(value, dict) and seg in value:
            value = value[seg]
        elif isinstance(value, list) and seg.isdigit() and int(seg) < len(value):
            value = value[int(seg)]
        else:
            return MISSING
    return value


def lookup(name, stack):
    if name == ".":
        for obj, idx in reversed(stack):
            if idx is not None:
                return obj
        return MISSING
    if name == "@index":
        for obj, idx in reversed(stack):
            if idx is not None:
                return idx
        return MISSING
    parts = name.split(".")
    first = parts[0]
    for obj, _idx in reversed(stack):
        if isinstance(obj, dict) and first in obj:
            return resolve_path(obj[first], parts[1:])
    return MISSING


def truthy(v):
    if v is MISSING or v is None or v is False:
        return False
    if isinstance(v, bool):
        return v
    if isinstance(v, (int, float)) and v == 0:
        return False
    if isinstance(v, (str, list)) and len(v) == 0:
        return False
    return True


def apply_filters(name_expr, stack):
    pieces = name_expr.split("|")
    value = lookup(pieces[0].strip(), stack)
    for f in pieces[1:]:
        f = f.strip()
        if f == "upper":
            if isinstance(value, str):
                value = value.upper()
            elif value is not MISSING and value is not None:
                value = format_value(value).upper()
        elif f == "lower":
            if isinstance(value, str):
                value = value.lower()
            elif value is not MISSING and value is not None:
                value = format_value(value).lower()
        elif f == "len":
            if isinstance(value, (str, list, dict)):
                value = len(value)
            else:
                value = MISSING
        elif f.startswith("default:"):
            text = f[len("default:"):]
            if value is MISSING or value is None or value == "":
                value = text
    return value


def format_value(v):
    if v is MISSING or v is None:
        return ""
    if isinstance(v, str):
        return v
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        return json.dumps(v)
    return json.dumps(v, separators=(",", ":"))


def render(nodes, stack):
    out = []
    for node in nodes:
        kind = node[0]
        if kind == "text":
            out.append(node[1])
        elif kind == "var":
            out.append(format_value(apply_filters(node[1], stack)))
        elif kind == "each":
            arr = lookup(node[1], stack)
            if isinstance(arr, list):
                for i, el in enumerate(arr):
                    out.append(render(node[2], stack + [(el, i)]))
        elif kind == "if":
            cond = lookup(node[1], stack)
            if truthy(cond):
                out.append(render(node[2], stack))
            elif node[3] is not None:
                out.append(render(node[3], stack))
    return "".join(out)


def main():
    data = sys.stdin.read()
    if "\n" in data:
        first, rest = data.split("\n", 1)
    else:
        first, rest = data, ""
    obj = json.loads(first)
    tree = parse(rest)
    sys.stdout.write(render(tree, [(obj, None)]))


if __name__ == "__main__":
    main()
