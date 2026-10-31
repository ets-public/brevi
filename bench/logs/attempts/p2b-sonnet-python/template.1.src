import sys, re, json

raw = sys.stdin.read()
if '\n' in raw:
    first, tpl = raw.split('\n', 1)
else:
    first, tpl = raw, ''
data = json.loads(first)

MISSING = object()
TAG = re.compile(r'\{\{(.*?)\}\}', re.S)

# parse into tree
def parse(tpl):
    root = []
    stack = [(None, root)]
    pos = 0
    for m in TAG.finditer(tpl):
        if m.start() > pos:
            stack[-1][1].append(('text', tpl[pos:m.start()]))
        pos = m.end()
        t = m.group(1).strip()
        if t.startswith('#each '):
            node = ['each', t[6:].strip(), []]
            stack[-1][1].append(node)
            stack.append((node, node[2]))
        elif t.startswith('#if '):
            node = ['if', t[4:].strip(), [], []]
            stack[-1][1].append(node)
            stack.append((node, node[2]))
        elif t == 'else':
            node = stack[-1][0]
            if node and node[0] == 'if':
                stack[-1] = (node, node[3])
        elif t == '/each' or t == '/if':
            if len(stack) > 1:
                stack.pop()
        else:
            stack[-1][1].append(('var', t))
    if pos < len(tpl):
        stack[-1][1].append(('text', tpl[pos:]))
    return root

def lookup(name, scopes):
    # scopes: list of (value, index), innermost last
    if name == '.':
        return scopes[-1][0] if scopes else data
    if name == '@index':
        for v, idx in reversed(scopes):
            if idx is not None:
                return idx
        return MISSING
    parts = name.split('.')
    first = parts[0]
    cur = MISSING
    for v, idx in reversed(scopes):
        if isinstance(v, dict) and first in v:
            cur = v[first]
            break
    else:
        if first in data:
            cur = data[first]
    if cur is MISSING:
        return MISSING
    for p in parts[1:]:
        if isinstance(cur, dict) and p in cur:
            cur = cur[p]
        elif isinstance(cur, list) and p.isdigit() and int(p) < len(cur):
            cur = cur[int(p)]
        else:
            return MISSING
    return cur

def tostr(v):
    if v is MISSING or v is None:
        return ''
    if isinstance(v, str):
        return v
    if v is True:
        return 'true'
    if v is False:
        return 'false'
    return json.dumps(v, separators=(',', ':'), ensure_ascii=False)

def truthy(v):
    if v is MISSING or v is None or v is False:
        return False
    if isinstance(v, (int, float)) and v == 0:
        return False
    if v == '' or v == []:
        return False
    return True

def evalvar(t, scopes):
    parts = t.split('|')
    v = lookup(parts[0].strip(), scopes)
    for f in parts[1:]:
        f = f.strip()
        if f == 'upper':
            v = tostr(v).upper()
        elif f == 'lower':
            v = tostr(v).lower()
        elif f == 'len':
            v = len(v) if isinstance(v, (str, list)) else 0
        elif f.startswith('default:'):
            if v is MISSING or v is None or v == '':
                v = f[8:]
    return tostr(v)

def render(nodes, scopes, out):
    for n in nodes:
        if n[0] == 'text':
            out.append(n[1])
        elif n[0] == 'var':
            out.append(evalvar(n[1], scopes))
        elif n[0] == 'each':
            v = lookup(n[1], scopes)
            if isinstance(v, list):
                for i, el in enumerate(v):
                    render(n[2], scopes + [(el, i)], out)
        elif n[0] == 'if':
            v = lookup(n[1], scopes)
            render(n[2] if truthy(v) else n[3], scopes, out)

out = []
render(parse(tpl), [], out)
sys.stdout.write(''.join(out))
