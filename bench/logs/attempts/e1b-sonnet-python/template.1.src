import sys, json, re
data = sys.stdin.read()
first, _, tpl = data.partition('\n')
root = json.loads(first)
MISSING = object()
tok = re.compile(r'\{\{(.*?)\}\}', re.S)
def parse(tpl):
    root_n = []; stack = [(None, root_n, None)]
    pos = 0
    cur = root_n
    for m in tok.finditer(tpl):
        if m.start() > pos:
            cur.append(('t', tpl[pos:m.start()]))
        pos = m.end()
        s = m.group(1).strip()
        if s.startswith('#each ') or s.startswith('#if '):
            kind, name = s[1:].split(None, 1)
            node = [kind, name.strip(), [], []]
            cur.append(node)
            stack.append((node, cur, 'body'))
            cur = node[2]
        elif s == 'else':
            node = stack[-1][0]
            cur = node[3]
        elif s in ('/each', '/if'):
            node, parent, _ = stack.pop()
            cur = parent
        else:
            cur.append(('v', s))
    if pos < len(tpl):
        cur.append(('t', tpl[pos:]))
    return root_n
def lookup(path, ctx):
    if path == '.':
        return ctx[-1][0]
    if path == '@index':
        return ctx[-1][1]
    segs = path.split('.')
    for el, idx in reversed(ctx):
        if isinstance(el, dict) and segs[0] in el:
            v = el
            break
    else:
        return MISSING
    for s in segs:
        if isinstance(v, dict) and s in v:
            v = v[s]
        else:
            return MISSING
    return v
def text(v):
    if v is MISSING or v is None: return ''
    if v is True: return 'true'
    if v is False: return 'false'
    if isinstance(v, str): return v
    return json.dumps(v, separators=(',', ':'), ensure_ascii=False)
def truthy(v):
    if v is MISSING or v is None or v is False: return False
    if isinstance(v, (int, float)) and v == 0: return False
    if v == '' or v == []: return False
    return True
def render_value(s, ctx):
    parts = s.split('|')
    v = lookup(parts[0].strip(), ctx)
    for f in parts[1:]:
        f = f.strip()
        if f == 'upper': v = text(v).upper()
        elif f == 'lower': v = text(v).lower()
        elif f == 'len':
            v = len(v) if isinstance(v, (str, list, dict)) else 0
        elif f.startswith('default:'):
            if v is MISSING or v is None or v == '':
                v = f[8:]
    return text(v)
def render(nodes, ctx):
    out = []
    for n in nodes:
        if n[0] == 't': out.append(n[1])
        elif n[0] == 'v': out.append(render_value(n[1], ctx))
        elif n[0] == 'each':
            v = lookup(n[1], ctx)
            if isinstance(v, list):
                for i, el in enumerate(v):
                    out.append(render(n[2], ctx + [(el, i)]))
        else:
            v = lookup(n[1], ctx)
            out.append(render(n[2] if truthy(v) else n[3], ctx))
    return ''.join(out)
sys.stdout.write(render(parse(tpl), [(root, None)]))
