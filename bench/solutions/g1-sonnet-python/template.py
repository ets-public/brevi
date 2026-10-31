import sys, re, json

raw = sys.stdin.read()
nl = raw.find('\n')
if nl < 0:
    first, tpl = raw, ''
else:
    first, tpl = raw[:nl], raw[nl + 1:]
data = json.loads(first)

MISSING = object()
TOK = re.compile(r'\{\{(.*?)\}\}', re.S)

def parse(tpl):
    toks = []
    pos = 0
    for m in TOK.finditer(tpl):
        if m.start() > pos:
            toks.append(('text', tpl[pos:m.start()]))
        toks.append(('tag', m.group(1).strip()))
        pos = m.end()
    if pos < len(tpl):
        toks.append(('text', tpl[pos:]))
    i = [0]
    def block(stop):
        nodes = []
        while i[0] < len(toks):
            k, v = toks[i[0]]
            if k == 'text':
                nodes.append(('text', v)); i[0] += 1; continue
            if v.startswith('#each '):
                i[0] += 1
                body, _ = block(('/each',))
                nodes.append(('each', v[6:].strip(), body))
            elif v.startswith('#if '):
                i[0] += 1
                body, end = block(('else', '/if'))
                other = []
                if end == 'else':
                    other, _ = block(('/if',))
                nodes.append(('if', v[4:].strip(), body, other))
            elif v in stop:
                i[0] += 1
                return nodes, v
            else:
                nodes.append(('var', v)); i[0] += 1
        return nodes, None
    return block(())[0]

def lookup(path, scopes):
    path = path.strip()
    if path == '.':
        return scopes[-1][0]
    if path == '@index':
        return scopes[-1][1]
    parts = path.split('.')
    for sc, _ in reversed(scopes):
        if isinstance(sc, dict) and parts[0] in sc:
            cur = sc[parts[0]]
            ok = True
            for p in parts[1:]:
                if isinstance(cur, dict) and p in cur:
                    cur = cur[p]
                else:
                    ok = False; break
            return cur if ok else MISSING
    return MISSING

def truthy(v):
    if v is MISSING or v is None or v is False:
        return False
    if v == 0 and not isinstance(v, str):
        return False
    if v == '' or v == []:
        return False
    return True

def tostr(v):
    if v is MISSING or v is None:
        return ''
    if isinstance(v, bool):
        return 'true' if v else 'false'
    if isinstance(v, str):
        return v
    return json.dumps(v, separators=(',', ':'), ensure_ascii=False)

def var(expr, scopes):
    parts = expr.split('|')
    v = lookup(parts[0], scopes)
    for f in parts[1:]:
        f = f.strip()
        if f == 'upper':
            v = tostr(v).upper()
        elif f == 'lower':
            v = tostr(v).lower()
        elif f == 'len':
            v = len(v) if isinstance(v, (str, list, dict)) else 0
        elif f.startswith('default:'):
            if v is MISSING or v is None or v == '':
                v = f[8:]
    return tostr(v)

def render(nodes, scopes, out):
    for n in nodes:
        if n[0] == 'text':
            out.append(n[1])
        elif n[0] == 'var':
            out.append(var(n[1], scopes))
        elif n[0] == 'each':
            arr = lookup(n[1], scopes)
            if isinstance(arr, list):
                for idx, el in enumerate(arr):
                    render(n[2], scopes + [(el, idx)], out)
        else:
            if truthy(lookup(n[1], scopes)):
                render(n[2], scopes, out)
            else:
                render(n[3], scopes, out)

out = []
render(parse(tpl), [(data, 0)], out)
sys.stdout.write(''.join(out))
