import sys, re, json

raw = sys.stdin.read()
if "\n" in raw:
    first, tpl = raw.split("\n", 1)
else:
    first, tpl = raw, ""
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
        k, v = toks[pos]
        if k == "text":
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
        elif v in ("else", "/if", "/each"):
            continue
        else:
            nodes.append(("var", v))
    return nodes, None


tree, _ = parse(())
MISSING = object()


def lookup(name, stack):
    if name == ".":
        return stack[-1][0]
    if name == "@index":
        return stack[-1][1]
    keys = name.split(".")
    for ctx, idx in reversed(stack):
        if isinstance(ctx, dict) and keys[0] in ctx:
            cur = ctx
            for k in keys:
                if isinstance(cur, dict) and k in cur:
                    cur = cur[k]
                else:
                    return MISSING
            return cur
    return MISSING


def tostr(v):
    if v is MISSING or v is None:
        return ""
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return v
    return json.dumps(v, separators=(',', ':'), ensure_ascii=False)


def truthy(v):
    if v is MISSING or v is None or v is False:
        return False
    if isinstance(v, (int, float)) and v == 0:
        return False
    if v == "" or v == []:
        return False
    return True


def var(expr, stack):
    segs = expr.split("|")
    v = lookup(segs[0].strip(), stack)
    for f in segs[1:]:
        f = f.strip()
        if f == "upper":
            v = tostr(v).upper()
        elif f == "lower":
            v = tostr(v).lower()
        elif f == "len":
            v = len(v) if isinstance(v, (str, list)) else 0
        elif f.startswith("default:"):
            if v is MISSING or v is None or v == "":
                v = f[8:]
    return tostr(v)


def render(nodes, stack, out):
    for n in nodes:
        t = n[0]
        if t == "text":
            out.append(n[1])
        elif t == "var":
            out.append(var(n[1], stack))
        elif t == "each":
            v = lookup(n[1], stack)
            if isinstance(v, list):
                for i, el in enumerate(v):
                    render(n[2], stack + [(el, i)], out)
        else:
            v = lookup(n[1], stack)
            render(n[2] if truthy(v) else n[3], stack, out)


out = []
render(tree, [(data, 0)], out)
sys.stdout.write("".join(out))
