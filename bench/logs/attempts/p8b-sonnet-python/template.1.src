import sys, re, json

data = sys.stdin.read()
nl = data.find("\n")
if nl < 0:
    first, tpl = data, ""
else:
    first, tpl = data[:nl], data[nl + 1:]
root = json.loads(first)

MISSING = object()
tagre = re.compile(r'\{\{(.*?)\}\}', re.S)

# parse
def parse(tpl):
    pos = 0
    stack = [[]]  # list of node lists
    heads = []
    for m in tagre.finditer(tpl):
        if m.start() > pos:
            stack[-1].append(("t", tpl[pos:m.start()]))
        pos = m.end()
        tag = m.group(1).strip()
        if tag.startswith("#each "):
            node = ["each", tag[6:].strip(), []]
            stack[-1].append(node)
            heads.append(node)
            stack.append(node[2])
        elif tag.startswith("#if "):
            node = ["if", tag[4:].strip(), [], []]
            stack[-1].append(node)
            heads.append(node)
            stack.append(node[2])
        elif tag == "else" and heads and heads[-1][0] == "if":
            stack[-1] = heads[-1][3]
        elif tag in ("/each", "/if") and heads:
            heads.pop()
            stack.pop()
        else:
            stack[-1].append(("v", tag))
    if pos < len(tpl):
        stack[0].append(("t", tpl[pos:]))
    return stack[0]

def lookup(name, scopes):
    if name == ".":
        return scopes[-1][0]
    if name == "@index":
        return scopes[-1][1]
    segs = name.split(".")
    for sc, _ in reversed(scopes):
        if isinstance(sc, dict) and segs[0] in sc:
            cur = sc
            ok = True
            for s in segs:
                if isinstance(cur, dict) and s in cur:
                    cur = cur[s]
                else:
                    ok = False
                    break
            return cur if ok else MISSING
    return MISSING

def tostr(v):
    if v is MISSING or v is None:
        return ""
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return v
    if isinstance(v, (int, float)):
        return json.dumps(v)
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)

def truthy(v):
    if v is MISSING or v is None or v is False:
        return False
    if isinstance(v, (int, float)) and v == 0:
        return False
    if v == "" or v == []:
        return False
    return True

def evalvar(tag, scopes):
    parts = tag.split("|")
    v = lookup(parts[0].strip(), scopes)
    for f in parts[1:]:
        f = f.strip()
        if f == "upper":
            v = tostr(v).upper()
        elif f == "lower":
            v = tostr(v).lower()
        elif f == "len":
            if isinstance(v, (str, list, dict)):
                v = len(v)
            else:
                v = 0
        elif f.startswith("default:"):
            if v is MISSING or v is None or v == "":
                v = f[8:]
    return tostr(v)

def render(nodes, scopes, out):
    for n in nodes:
        k = n[0]
        if k == "t":
            out.append(n[1])
        elif k == "v":
            out.append(evalvar(n[1], scopes))
        elif k == "each":
            v = lookup(n[1], scopes)
            if isinstance(v, list):
                for i, el in enumerate(v):
                    render(n[2], scopes + [(el, i)], out)
        elif k == "if":
            v = lookup(n[1], scopes)
            render(n[2] if truthy(v) else n[3], scopes, out)

out = []
render(parse(tpl), [(root, 0)], out)
sys.stdout.write("".join(out))
