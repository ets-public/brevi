import sys, json, re
raw = sys.stdin.read()
first, _, tpl = raw.partition('\n')
data = json.loads(first)
MISSING = object()
toks = re.split(r'(\{\{.*?\}\})', tpl, flags=re.S)
pos = 0
def parse(end):
    global pos
    nodes = []
    while pos < len(toks):
        t = toks[pos]; pos += 1
        if t.startswith('{{') and t.endswith('}}'):
            inner = t[2:-2].strip()
            if inner.startswith('#each '):
                body, _ = parse(('/each',))
                nodes.append(('each', inner[6:].strip(), body))
            elif inner.startswith('#if '):
                body, e = parse(('/if', 'else'))
                eb = []
                if e == 'else':
                    eb, _ = parse(('/if',))
                nodes.append(('if', inner[4:].strip(), body, eb))
            elif inner in end:
                return nodes, inner
            else:
                nodes.append(('var', inner))
        elif t:
            nodes.append(('text', t))
    return nodes, None
def lookup(name, scopes):
    if name == '.':
        return scopes[0][0]
    if name == '@index':
        return scopes[0][1]
    parts = name.split('.')
    for el, _ in scopes:
        if isinstance(el, dict) and parts[0] in el:
            cur = el
            for p in parts:
                if isinstance(cur, dict) and p in cur:
                    cur = cur[p]
                else:
                    return MISSING
            return cur
    return MISSING
def show(v):
    if v is MISSING or v is None:
        return ''
    if isinstance(v, bool):
        return 'true' if v else 'false'
    if isinstance(v, str):
        return v
    return json.dumps(v, separators=(',', ':'), ensure_ascii=False)
def truthy(v):
    if v is MISSING or v is None or v is False:
        return False
    if isinstance(v, (int, float)) and v == 0:
        return False
    if v == '' or v == []:
        return False
    return True
def var(expr, scopes):
    parts = expr.split('|')
    v = lookup(parts[0].strip(), scopes)
    for f in parts[1:]:
        f = f.strip()
        if f == 'upper':
            v = show(v).upper()
        elif f == 'lower':
            v = show(v).lower()
        elif f == 'len':
            v = len(v) if isinstance(v, (str, list, dict)) else 0
        elif f.startswith('default:'):
            if v is MISSING or v is None or v == '':
                v = f[8:]
    return show(v)
def render(nodes, scopes):
    out = []
    for n in nodes:
        if n[0] == 'text':
            out.append(n[1])
        elif n[0] == 'var':
            out.append(var(n[1], scopes))
        elif n[0] == 'each':
            arr = lookup(n[1], scopes)
            if isinstance(arr, list):
                for i, e in enumerate(arr):
                    out.append(render(n[2], [(e, i)] + scopes))
        else:
            if truthy(lookup(n[1], scopes)):
                out.append(render(n[2], scopes))
            else:
                out.append(render(n[3], scopes))
    return ''.join(out)
nodes, _ = parse(())
sys.stdout.write(render(nodes, [(data, 0)]))
