import sys, json, re
data = sys.stdin.read()
first, _, tpl = data.partition('\n')
root = json.loads(first)
MISSING = object()
def tokens(s):
    out = []; p = 0
    for m in re.finditer(r'\{\{(.*?)\}\}', s, re.S):
        if m.start() > p: out.append(('t', s[p:m.start()]))
        out.append(('g', m.group(1).strip())); p = m.end()
    if p < len(s): out.append(('t', s[p:]))
    return out
def parse(toks, i, end):
    nodes = []
    while i < len(toks):
        k, v = toks[i]
        if k == 't':
            nodes.append(('t', v)); i += 1; continue
        if v in end:
            return nodes, i, v
        if v.startswith('#each '):
            body, i, _ = parse(toks, i + 1, ('/each',))
            nodes.append(('each', v[6:].strip(), body)); i += 1
        elif v.startswith('#if '):
            body, i, e = parse(toks, i + 1, ('else', '/if'))
            els = []
            if e == 'else':
                els, i, _ = parse(toks, i + 1, ('/if',))
            nodes.append(('if', v[4:].strip(), body, els)); i += 1
        else:
            nodes.append(('v', v)); i += 1
    return nodes, i, None
def walk(obj, path):
    for p in path:
        if isinstance(obj, dict) and p in obj: obj = obj[p]
        else: return MISSING
    return obj
def lookup(name, scopes):
    # scopes: list of (elem, index), innermost last
    if name == '@index':
        return scopes[-1][1] if scopes and scopes[-1][1] is not None else MISSING
    if name == '.':
        return scopes[-1][0] if scopes else MISSING
    path = name.split('.')
    for el, idx in reversed(scopes):
        if isinstance(el, dict):
            v = walk(el, path)
            if v is not MISSING: return v
    return walk(root, path)
def tostr(v):
    if v is MISSING or v is None: return ''
    if isinstance(v, str): return v
    if v is True: return 'true'
    if v is False: return 'false'
    return json.dumps(v, separators=(',', ':'), ensure_ascii=False)
def truthy(v):
    if v is MISSING or v is None or v is False: return False
    if isinstance(v, (int, float)) and v == 0: return False
    if v == '' or v == []: return False
    return True
def value(expr, scopes):
    parts = expr.split('|')
    v = lookup(parts[0].strip(), scopes)
    for f in parts[1:]:
        f = f.strip()
        if f == 'upper': v = tostr(v).upper()
        elif f == 'lower': v = tostr(v).lower()
        elif f == 'len':
            v = len(v) if isinstance(v, (str, list)) else 0
        elif f.startswith('default:'):
            if v is MISSING or v is None or v == '': v = f[8:]
    return tostr(v)
def render(nodes, scopes, out):
    for n in nodes:
        if n[0] == 't': out.append(n[1])
        elif n[0] == 'v': out.append(value(n[1], scopes))
        elif n[0] == 'each':
            arr = lookup(n[1], scopes)
            if isinstance(arr, list):
                for i, e in enumerate(arr):
                    render(n[2], scopes + [(e, i)], out)
        else:
            render(n[2] if truthy(lookup(n[1], scopes)) else n[3], scopes, out)
nodes, _, _ = parse(tokens(tpl), 0, ())
out = []
render(nodes, [], out)
sys.stdout.write(''.join(out))
