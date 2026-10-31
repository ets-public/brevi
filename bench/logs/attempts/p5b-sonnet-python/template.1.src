import sys, re, json
raw = sys.stdin.read()
nl = raw.find("\n")
if nl < 0:
    first, tpl = raw, ""
else:
    first, tpl = raw[:nl], raw[nl + 1:]
data = json.loads(first)
MISSING = object()
parts = re.split(r"(\{\{.*?\}\})", tpl, flags=re.S)
pos = 0
def parse(end_tags):
    global pos
    nodes = []
    while pos < len(parts):
        p = parts[pos]
        if p.startswith("{{") and p.endswith("}}") and len(p) >= 4:
            inner = p[2:-2].strip()
            if inner in end_tags:
                return nodes, inner
            pos += 1
            if inner.startswith("#each "):
                body, _ = parse(("/each",))
                pos += 1
                nodes.append(("each", inner[6:].strip(), body))
            elif inner.startswith("#if "):
                body, t = parse(("else", "/if"))
                els = []
                if t == "else":
                    pos += 1
                    els, _ = parse(("/if",))
                pos += 1
                nodes.append(("if", inner[4:].strip(), body, els))
            else:
                nodes.append(("var", inner))
        else:
            if p:
                nodes.append(("text", p))
            pos += 1
    return nodes, None
def lookup(name, scopes):
    if name == ".":
        return scopes[-1][0]
    if name == "@index":
        return scopes[-1][1]
    segs = name.split(".")
    for sc in reversed(scopes):
        el = sc[0]
        if isinstance(el, dict) and segs[0] in el:
            return walk(el, segs)
    return MISSING
def walk(cur, segs):
    for s in segs:
        if isinstance(cur, dict) and s in cur:
            cur = cur[s]
        else:
            return MISSING
    return cur
def tostr(v):
    if v is MISSING or v is None: return ""
    if isinstance(v, str): return v
    if v is True: return "true"
    if v is False: return "false"
    if isinstance(v, float) and v == int(v): return str(int(v))
    if isinstance(v, (int, float)): return json.dumps(v)
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)
def truthy(v):
    if v is MISSING or v is None or v is False: return False
    if isinstance(v, (int, float)) and v == 0: return False
    if v == "" or v == []: return False
    return True
def evalvar(inner, scopes):
    segs = inner.split("|")
    v = lookup(segs[0].strip(), scopes)
    for f in segs[1:]:
        f = f.strip()
        if f == "upper": v = tostr(v).upper()
        elif f == "lower": v = tostr(v).lower()
        elif f == "len":
            v = len(v) if isinstance(v, (str, list)) else 0
        elif f.startswith("default:"):
            if v is MISSING or v is None or v == "":
                v = f[8:]
    return tostr(v)
def render(nodes, scopes, out):
    for n in nodes:
        if n[0] == "text": out.append(n[1])
        elif n[0] == "var": out.append(evalvar(n[1], scopes))
        elif n[0] == "each":
            v = lookup(n[1], scopes)
            if isinstance(v, list):
                for i, e in enumerate(v):
                    render(n[2], scopes + [(e, i)], out)
        else:
            v = lookup(n[1], scopes)
            render(n[2] if truthy(v) else n[3], scopes, out)
nodes, _ = parse(())
out = []
render(nodes, [(data, 0)], out)
sys.stdout.write("".join(out))
