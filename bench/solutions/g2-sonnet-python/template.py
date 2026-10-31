import sys, re, json
raw = sys.stdin.read()
nl = raw.find("\n")
if nl < 0: first, tpl = raw, ""
else: first, tpl = raw[:nl], raw[nl + 1:]
data = json.loads(first)
parts = re.split(r"(\{\{.*?\}\})", tpl, flags=re.S)
def parse(parts):
    root = []
    stack = [(None, root, None)]  # (kind/expr info, current list, extra)
    cur = root
    st = []  # entries: [node, listref]
    for p in parts:
        if p.startswith("{{") and p.endswith("}}") and len(p) >= 4:
            t = p[2:-2].strip()
            if t.startswith("#each "):
                node = ["each", t[6:].strip(), []]
                cur.append(node); st.append((node, cur)); cur = node[2]
            elif t.startswith("#if "):
                node = ["if", t[4:].strip(), [], []]
                cur.append(node); st.append((node, cur)); cur = node[2]
            elif t == "else" and st and st[-1][0][0] == "if":
                cur = st[-1][0][3]
            elif t in ("/each", "/if") and st:
                node, cur = st.pop()
            else:
                cur.append(["var", t])
        elif p:
            cur.append(["text", p])
    return root
tree = parse(parts)
MISSING = None
def lookup(name, scopes):
    if name == ".":
        return scopes[-1][0] if scopes else data
    if name == "@index":
        return scopes[-1][1] if scopes else None
    segs = name.split(".")
    first = segs[0]
    val = None; found = False
    for el, _ in reversed(scopes):
        if isinstance(el, dict) and first in el:
            val = el[first]; found = True; break
    if not found:
        if isinstance(data, dict) and first in data:
            val = data[first]; found = True
    if not found: return None
    for s in segs[1:]:
        if isinstance(val, dict) and s in val: val = val[s]
        elif isinstance(val, list) and s.isdigit() and int(s) < len(val): val = val[int(s)]
        else: return None
    return val
def text(v):
    if v is None: return ""
    if v is True: return "true"
    if v is False: return "false"
    if isinstance(v, str): return v
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)
def evalexpr(expr, scopes):
    fs = expr.split("|")
    v = lookup(fs[0].strip(), scopes)
    for f in fs[1:]:
        f = f.strip()
        if f == "upper": v = text(v).upper()
        elif f == "lower": v = text(v).lower()
        elif f == "len":
            v = len(v) if isinstance(v, (str, list, dict)) else 0
        elif f.startswith("default:"):
            if v is None or v == "": v = f[8:]
    return v
def truthy(v):
    if v is None or v is False or v == 0 or v == "" or v == []: return False
    return True
def render(nodes, scopes, out):
    for n in nodes:
        k = n[0]
        if k == "text": out.append(n[1])
        elif k == "var": out.append(text(evalexpr(n[1], scopes)))
        elif k == "each":
            v = lookup(n[1], scopes)
            if isinstance(v, list):
                for i, el in enumerate(v):
                    render(n[2], scopes + [(el, i)], out)
        elif k == "if":
            render(n[2] if truthy(evalexpr(n[1], scopes)) else n[3], scopes, out)
out = []
render(tree, [], out)
sys.stdout.write("".join(out))
