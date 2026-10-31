import json
import re
import sys

MISSING = object()
TAG_SPLIT = re.compile(r'(\{\{.*?\}\})', re.S)


def parse(tpl):
    root = []
    cur = root
    stack = []  # (node, parent_list)
    for idx, part in enumerate(TAG_SPLIT.split(tpl)):
        if idx % 2 == 0:
            if part:
                cur.append(['text', part])
            continue
        s = part[2:-2].strip()
        if s.startswith('#each '):
            node = ['each', s[6:].strip(), []]
            cur.append(node)
            stack.append((node, cur))
            cur = node[2]
        elif s.startswith('#if '):
            node = ['if', s[4:].strip(), [], None]
            cur.append(node)
            stack.append((node, cur))
            cur = node[2]
        elif s == 'else' and stack and stack[-1][0][0] == 'if':
            node = stack[-1][0]
            node[3] = []
            cur = node[3]
        elif s in ('/each', '/if'):
            if stack:
                _, parent = stack.pop()
                cur = parent
        else:
            cur.append(['var', s])
    return root


def lookup_in(obj, parts):
    if not isinstance(obj, dict) or parts[0] not in obj:
        return MISSING
    val = obj[parts[0]]
    for p in parts[1:]:
        if isinstance(val, dict) and p in val:
            val = val[p]
        else:
            return MISSING
    return val


def lookup(name, scopes):
    if name == '.':
        for frame in reversed(scopes):
            if frame[0] == 'elem':
                return frame[1]
        return MISSING
    if name.startswith('@'):
        key = name[1:]
        for frame in reversed(scopes):
            if frame[0] == 'elem':
                return frame[2] if key == 'index' else MISSING
        return MISSING
    parts = name.split('.')
    for frame in reversed(scopes):
        obj = frame[1] if frame[0] == 'obj' else frame[1]
        val = lookup_in(obj, parts)
        if val is not MISSING:
            return val
    return MISSING


def to_text(v):
    if v is None or v is MISSING:
        return ''
    if isinstance(v, bool):
        return 'true' if v else 'false'
    if isinstance(v, str):
        return v
    if isinstance(v, (int, float)):
        return json.dumps(v)
    return json.dumps(v, separators=(',', ':'))


def truthy(v):
    if v is MISSING or v is None or v is False:
        return False
    if isinstance(v, bool):
        return v
    if isinstance(v, (int, float)) and v == 0:
        return False
    if isinstance(v, (str, list)) and len(v) == 0:
        return False
    return True


def apply_filter(val, f):
    if ':' in f:
        fname, arg = f.split(':', 1)
    else:
        fname, arg = f, None
    fname = fname.strip()
    if fname == 'upper':
        return to_text(val).upper()
    if fname == 'lower':
        return to_text(val).lower()
    if fname == 'len':
        if isinstance(val, (str, list, dict)):
            return len(val)
        return None
    if fname == 'default':
        if val is MISSING or val is None or val == '':
            return arg if arg is not None else ''
        return val
    return val


def render_var(expr, scopes):
    segs = [s.strip() for s in expr.split('|')]
    val = lookup(segs[0], scopes)
    if val is MISSING:
        val = None
    for f in segs[1:]:
        val = apply_filter(val, f)
    return to_text(val)


def render(nodes, scopes):
    out = []
    for node in nodes:
        kind = node[0]
        if kind == 'text':
            out.append(node[1])
        elif kind == 'var':
            out.append(render_var(node[1], scopes))
        elif kind == 'each':
            arr = lookup(node[1], scopes)
            if isinstance(arr, list):
                for i, el in enumerate(arr):
                    out.append(render(node[2], scopes + [('elem', el, i)]))
        elif kind == 'if':
            if truthy(lookup(node[1], scopes)):
                out.append(render(node[2], scopes))
            elif node[3] is not None:
                out.append(render(node[3], scopes))
    return ''.join(out)


def main():
    text = sys.stdin.read()
    first, _, tpl = text.partition('\n')
    data = json.loads(first)
    result = render(parse(tpl), [('obj', data)])
    sys.stdout.write(result if result.endswith('\n') else result + '\n')


if __name__ == '__main__':
    main()
