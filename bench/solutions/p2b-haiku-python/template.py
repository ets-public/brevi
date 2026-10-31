import json
import re
import sys

MISSING = object()
TAG_RE = re.compile(r"\{\{(.*?)\}\}")


def parse(tokens, pos):
    nodes = []
    while pos < len(tokens):
        kind, val = tokens[pos]
        if kind == "text":
            nodes.append(("text", val))
            pos += 1
            continue
        t = val
        if t in ("else", "/each", "/if"):
            return nodes, pos, t
        if t.startswith("#each "):
            name = t[6:].strip()
            body, pos, _ = parse(tokens, pos + 1)
            nodes.append(("each", name, body))
            pos += 1
            continue
        if t.startswith("#if "):
            name = t[4:].strip()
            then, pos, stop = parse(tokens, pos + 1)
            els = []
            if stop == "else":
                els, pos, _ = parse(tokens, pos + 1)
            nodes.append(("if", name, then, els))
            pos += 1
            continue
        nodes.append(("var", t))
        pos += 1
    return nodes, pos, None


def resolve(value, parts):
    for p in parts:
        if isinstance(value, dict) and p in value:
            value = value[p]
        else:
            return MISSING
    return value


def lookup(name, frames):
    if name == "@index":
        for value, index in reversed(frames):
            if index is not None:
                return index
        return MISSING
    if name == ".":
        return frames[-1][0]
    parts = name.split(".")
    for value, _ in reversed(frames):
        r = resolve(value, parts)
        if r is not MISSING:
            return r
    return MISSING


def plain(v):
    if v is MISSING or v is None:
        return ""
    if v is True:
        return "true"
    if v is False:
        return "false"
    if isinstance(v, str):
        return v
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        return json.dumps(v)
    return json.dumps(v, separators=(",", ":"))


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


def eval_expr(expr, frames):
    segs = expr.split("|")
    v = lookup(segs[0].strip(), frames)
    for f in segs[1:]:
        f = f.strip()
        if f == "upper":
            if v is not MISSING and v is not None:
                v = plain(v).upper()
        elif f == "lower":
            if v is not MISSING and v is not None:
                v = plain(v).lower()
        elif f == "len":
            if isinstance(v, (str, list, dict)):
                v = len(v)
            else:
                v = MISSING
        elif f.startswith("default:"):
            if v is MISSING or v is None or v == "":
                v = f[len("default:"):]
    return v


def render(nodes, frames, out):
    for node in nodes:
        kind = node[0]
        if kind == "text":
            out.append(node[1])
        elif kind == "var":
            out.append(plain(eval_expr(node[1], frames)))
        elif kind == "each":
            seq = lookup(node[1], frames)
            if isinstance(seq, list):
                for i, item in enumerate(seq):
                    render(node[2], frames + [(item, i)], out)
        elif kind == "if":
            if truthy(lookup(node[1], frames)):
                render(node[2], frames, out)
            else:
                render(node[3], frames, out)


def main():
    data = sys.stdin.read()
    first, _, template = data.partition("\n")
    obj = json.loads(first)
    tokens = []
    pos = 0
    for m in TAG_RE.finditer(template):
        if m.start() > pos:
            tokens.append(("text", template[pos:m.start()]))
        tokens.append(("tag", m.group(1).strip()))
        pos = m.end()
    if pos < len(template):
        tokens.append(("text", template[pos:]))
    nodes, _, _ = parse(tokens, 0)
    out = []
    render(nodes, [(obj, None)], out)
    sys.stdout.write("".join(out))


if __name__ == "__main__":
    main()
