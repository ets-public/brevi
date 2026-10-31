import sys
import json
import re

MISSING = object()
TAG = re.compile(r'\{\{(.*?)\}\}')


def tokenize(s):
    toks = []
    pos = 0
    for m in TAG.finditer(s):
        if m.start() > pos:
            toks.append(('text', s[pos:m.start()]))
        toks.append(('tag', m.group(1).strip()))
        pos = m.end()
    if pos < len(s):
        toks.append(('text', s[pos:]))
    return toks


def parse(toks, i, terms):
    nodes = []
    while i < len(toks):
        kind, val = toks[i]
        if kind == 'text':
            nodes.append(('text', val))
            i += 1
            continue
        if val in terms:
            return nodes, i, val
        if val.startswith('#each '):
            name = val[6:].strip()
            body, i, _ = parse(toks, i + 1, ('/each',))
            nodes.append(('each', name, body))
            i += 1
        elif val.startswith('#if '):
            name = val[4:].strip()
            body, i, term = parse(toks, i + 1, ('else', '/if'))
            els = []
            if term == 'else':
                els, i, term = parse(toks, i + 1, ('/if',))
            nodes.append(('if', name, body, els))
            i += 1
        else:
            nodes.append(('var', val))
            i += 1
    return nodes, i, None


def lookup_path(obj, parts):
    cur = obj
    for p in parts:
        if isinstance(cur, dict) and p in cur:
            cur = cur[p]
        else:
            return MISSING
    return cur


def resolve(name, scopes, data):
    if name == '.':
        return scopes[-1][0] if scopes else MISSING
    if name == '@index':
        return scopes[-1][1] if scopes else MISSING
    parts = name.split('.')
    for elem, _ in reversed(scopes):
        v = lookup_path(elem, parts)
        if v is not MISSING:
            return v
    return lookup_path(data, parts)


def to_text(v):
    if v is MISSING or v is None:
        return ''
    if isinstance(v, bool):
        return 'true' if v else 'false'
    if isinstance(v, str):
        return v
    if isinstance(v, (int, float)):
        return json.dumps(v)
    return json.dumps(v, separators=(',', ':'), ensure_ascii=False)


def truthy(v):
    if v is MISSING or v is None:
        return False
    if isinstance(v, bool):
        return v
    if isinstance(v, (int, float)):
        return v != 0
    if isinstance(v, (str, list)):
        return len(v) > 0
    return True


def eval_expr(expr, scopes, data):
    parts = [p.strip() for p in expr.split('|')]
    v = resolve(parts[0], scopes, data)
    for f in parts[1:]:
        fname, _, arg = f.partition(':')
        fname = fname.strip()
        if fname == 'default':
            if v is MISSING or v is None or v == '':
                v = arg
        elif fname == 'upper':
            v = to_text(v).upper()
        elif fname == 'lower':
            v = to_text(v).lower()
        elif fname == 'len':
            v = len(v) if isinstance(v, (str, list)) else MISSING
    return to_text(v)


def render(nodes, scopes, data):
    out = []
    for node in nodes:
        t = node[0]
        if t == 'text':
            out.append(node[1])
        elif t == 'var':
            out.append(eval_expr(node[1], scopes, data))
        elif t == 'each':
            arr = resolve(node[1], scopes, data)
            if isinstance(arr, list):
                for idx, elem in enumerate(arr):
                    out.append(render(node[2], scopes + [(elem, idx)], data))
        elif t == 'if':
            v = resolve(node[1], scopes, data)
            out.append(render(node[2] if truthy(v) else node[3], scopes, data))
    return ''.join(out)


def main():
    raw = sys.stdin.read()
    first, _, template = raw.partition('\n')
    data = json.loads(first)
    nodes, _, _ = parse(tokenize(template), 0, ())
    sys.stdout.write(render(nodes, [], data))


main()
