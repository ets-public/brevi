import json
import re
import sys

MISSING = object()
TAG_RE = re.compile(r'\{\{(.*?)\}\}')


def tokenize(text):
    toks = []
    pos = 0
    for m in TAG_RE.finditer(text):
        if m.start() > pos:
            toks.append(('text', text[pos:m.start()]))
        toks.append(('tag', m.group(1).strip()))
        pos = m.end()
    if pos < len(text):
        toks.append(('text', text[pos:]))
    return toks


def parse(toks, pos):
    nodes = []
    n = len(toks)
    while pos < n:
        kind, val = toks[pos]
        if kind == 'text':
            nodes.append(('text', val))
            pos += 1
            continue
        if val in ('/each', '/if', 'else'):
            return nodes, pos
        m = re.match(r'#each\s+(.*)$', val)
        if m:
            body, pos = parse(toks, pos + 1)
            nodes.append(('each', m.group(1).strip(), body))
            pos += 1
            continue
        m = re.match(r'#if\s+(.*)$', val)
        if m:
            then, pos = parse(toks, pos + 1)
            els = []
            if pos < n and toks[pos][1] == 'else':
                els, pos = parse(toks, pos + 1)
            nodes.append(('if', m.group(1).strip(), then, els))
            pos += 1
            continue
        nodes.append(('var', val))
        pos += 1
    return nodes, pos


def resolve(obj, name):
    cur = obj
    for part in name.split('.'):
        if isinstance(cur, dict) and part in cur:
            cur = cur[part]
        else:
            return MISSING
    return cur


def lookup(name, stack):
    if name == '@index':
        for _, idx in reversed(stack):
            if idx is not None:
                return idx
        return MISSING
    if name == '.':
        return stack[-1][0]
    for val, _ in reversed(stack):
        if isinstance(val, dict):
            r = resolve(val, name)
            if r is not MISSING:
                return r
    return MISSING


def render_value(v):
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
    if v is True:
        return True
    if isinstance(v, (int, float)):
        return v != 0
    if isinstance(v, (str, list)):
        return len(v) > 0
    return True


def render_expr(expr, stack):
    parts = [p.strip() for p in expr.split("|")]
    val = lookup(parts[0], stack)
    for f in parts[1:]:
        fname, _, arg = f.partition(":")
        fname = fname.strip()
        arg = arg.strip()
        if fname == "upper":
            if val is not MISSING and val is not None:
                val = render_value(val).upper()
        elif fname == "lower":
            if val is not MISSING and val is not None:
                val = render_value(val).lower()
        elif fname == "len":
            if isinstance(val, (str, list, dict)):
                val = len(val)
            else:
                val = MISSING
        elif fname == "default":
            if val is MISSING or val is None or val == "":
                val = arg
    return render_value(val)


def render(nodes, stack, out):
    for node in nodes:
        kind = node[0]
        if kind == "text":
            out.append(node[1])
        elif kind == "var":
            out.append(render_expr(node[1], stack))
        elif kind == "each":
            items = lookup(node[1], stack)
            if isinstance(items, list):
                for idx, el in enumerate(items):
                    render(node[2], stack + [(el, idx)], out)
        elif kind == "if":
            if truthy(lookup(node[1], stack)):
                render(node[2], stack, out)
            else:
                render(node[3], stack, out)


def main():
    text = sys.stdin.read()
    first, _, template = text.partition("\n")
    data = json.loads(first)
    toks = tokenize(template)

    nodes = []
    pos = 0
    while pos < len(toks):
        more, pos = parse(toks, pos)
        nodes.extend(more)
        if pos < len(toks):
            nodes.append(("text", "{{" + toks[pos][1] + "}}"))
            pos += 1

    out = []
    render(nodes, [(data, None)], out)
    sys.stdout.write("".join(out))


if __name__ == "__main__":
    main()
