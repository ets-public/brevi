import json
import re
import sys

MISSING = object()
TAG_RE = re.compile(r"\{\{(.*?)\}\}", re.S)


def parse(text):
    root = []
    stack = []  # (node, key)

    def cur():
        if not stack:
            return root
        node, key = stack[-1]
        return node[key]

    pos = 0
    for m in TAG_RE.finditer(text):
        if m.start() > pos:
            cur().append({"type": "text", "value": text[pos:m.start()]})
        pos = m.end()
        tag = m.group(1).strip()
        if tag.startswith("#each "):
            node = {"type": "each", "name": tag[6:].strip(), "body": []}
            cur().append(node)
            stack.append((node, "body"))
        elif tag.startswith("#if "):
            node = {"type": "if", "name": tag[4:].strip(), "body": [], "else_body": []}
            cur().append(node)
            stack.append((node, "body"))
        elif tag == "else" and stack and stack[-1][0]["type"] == "if":
            node, _ = stack[-1]
            stack[-1] = (node, "else_body")
        elif tag in ("/each", "/if") and stack:
            stack.pop()
        else:
            cur().append({"type": "expr", "expr": tag})
    if pos < len(text):
        cur().append({"type": "text", "value": text[pos:]})
    return root


def path_get(obj, path):
    for part in path.split("."):
        if isinstance(obj, dict) and part in obj:
            obj = obj[part]
        else:
            return MISSING
    return obj


def lookup(name, scopes, data):
    if name == ".":
        return scopes[-1][0] if scopes else MISSING
    if name == "@index":
        return scopes[-1][1] if scopes else MISSING
    for item, _ in reversed(scopes):
        if isinstance(item, dict):
            v = path_get(item, name)
            if v is not MISSING:
                return v
    return path_get(data, name)


def to_text(v):
    if v is MISSING or v is None:
        return ""
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return v
    if isinstance(v, int):
        return str(v)
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)


def truthy(v):
    if v is MISSING or v is None:
        return False
    if isinstance(v, bool):
        return v
    if isinstance(v, (int, float)):
        return v != 0
    if isinstance(v, (str, list)):
        return len(v) > 0
    return True


def eval_expr(expr, scopes, data):
    parts = expr.split("|")
    value = lookup(parts[0].strip(), scopes, data)
    for f in parts[1:]:
        f = f.strip()
        if f == "upper":
            value = to_text(value).upper()
        elif f == "lower":
            value = to_text(value).lower()
        elif f == "len":
            if isinstance(value, (str, list, dict)):
                value = len(value)
            else:
                value = MISSING
        elif f.startswith("default:"):
            text = f[len("default:"):]
            if value is MISSING or value is None or value == "":
                value = text
        # unknown filters leave the value unchanged
    return to_text(value)


def render(nodes, scopes, data, out):
    for node in nodes:
        t = node["type"]
        if t == "text":
            out.append(node["value"])
        elif t == "expr":
            out.append(eval_expr(node["expr"], scopes, data))
        elif t == "each":
            seq = lookup(node["name"], scopes, data)
            if isinstance(seq, list):
                for i, item in enumerate(seq):
                    scopes.append((item, i))
                    render(node["body"], scopes, data, out)
                    scopes.pop()
        elif t == "if":
            v = lookup(node["name"], scopes, data)
            if truthy(v):
                render(node["body"], scopes, data, out)
            else:
                render(node["else_body"], scopes, data, out)


def main():
    raw = sys.stdin.read()
    first, nl, template = raw.partition("\n")
    data = json.loads(first)
    nodes = parse(template)
    out = []
    render(nodes, [], data, out)
    result = "".join(out)
    sys.stdout.write(result if result.endswith("\n") or result == "" else result + "\n")


if __name__ == "__main__":
    main()
