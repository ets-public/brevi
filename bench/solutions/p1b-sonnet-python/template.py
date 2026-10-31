import sys, json, re

MISSING = object()
inp = sys.stdin.read()
first, _, tpl = inp.partition("\n")
data = json.loads(first)

parts = re.split(r'(\{\{.*?\}\})', tpl, flags=re.S)
toks = []
for p in parts:
    if p.startswith("{{") and p.endswith("}}") and len(p) >= 4:
        toks.append(("tag", p[2:-2].strip()))
    elif p != "":
        toks.append(("text", p))

pos = 0


def parse(stop):
    global pos
    nodes = []
    while pos < len(toks):
        kind, v = toks[pos]
        if kind == "text":
            nodes.append(("text", v))
            pos += 1
            continue
        if v in stop:
            return nodes, v
        pos += 1
        if v.startswith("#each "):
            body, _ = parse(("/each",))
            pos += 1
            nodes.append(("each", v[6:].strip(), body))
        elif v.startswith("#if "):
            body, end = parse(("else", "/if"))
            pos += 1
            other = []
            if end == "else":
                other, _ = parse(("/if",))
                pos += 1
            nodes.append(("if", v[4:].strip(), body, other))
        else:
            nodes.append(("var", v))
    return nodes, None


tree, _ = parse(())


def path_get(val, path):
    for seg in path:
        if isinstance(val, dict) and seg in val:
            val = val[seg]
        elif isinstance(val, list) and seg.isdigit() and int(seg) < len(val):
            val = val[int(seg)]
        else:
            return MISSING
    return val


def lookup(name, frames):
    if name == ".":
        return frames[-1][0]
    if name == "@index":
        for f in reversed(frames):
            if f[1] is not None:
                return f[1]
        return MISSING
    path = name.split(".")
    for val, idx in reversed(frames):
        if isinstance(val, dict) or val is frames[0][0]:
            r = path_get(val, path)
            if r is not MISSING:
                return r
    return MISSING


def fmt(v):
    if v is MISSING or v is None:
        return ""
    if v is True:
        return "true"
    if v is False:
        return "false"
    if isinstance(v, str):
        return v
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)


def truthy(v):
    if v is MISSING or v is None or v is False:
        return False
    if v == 0 or v == "" or v == []:
        return False
    return True


def evalvar(expr, frames):
    segs = expr.split("|")
    v = lookup(segs[0].strip(), frames)
    for f in segs[1:]:
        f = f.strip()
        if f == "upper":
            v = fmt(v).upper()
        elif f == "lower":
            v = fmt(v).lower()
        elif f == "len":
            v = len(v) if isinstance(v, (str, list, dict)) else 0
        elif f.startswith("default:"):
            if v is MISSING or v is None or v == "":
                v = f[8:]
    return fmt(v)


def render(nodes, frames, out):
    for n in nodes:
        if n[0] == "text":
            out.append(n[1])
        elif n[0] == "var":
            out.append(evalvar(n[1], frames))
        elif n[0] == "each":
            arr = lookup(n[1], frames)
            if isinstance(arr, list):
                for i, el in enumerate(arr):
                    render(n[2], frames + [(el, i)], out)
        elif n[0] == "if":
            if truthy(lookup(n[1], frames)):
                render(n[2], frames, out)
            else:
                render(n[3], frames, out)


out = []
render(tree, [(data, None)], out)
sys.stdout.write("".join(out))
