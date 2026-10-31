import sys
import re
import json

TAG = re.compile(r'\{\{(.*?)\}\}', re.S)


def tokenize(s):
    items = []
    pos = 0
    for m in TAG.finditer(s):
        if m.start() > pos:
            items.append(('t', s[pos:m.start()]))
        items.append(('g', m.group(1).strip()))
        pos = m.end()
    if pos < len(s):
        items.append(('t', s[pos:]))
    return items


def parse_seq(items, i):
    nodes = []
    while i < len(items):
        kind, val = items[i]
        if kind == 't':
            nodes.append(('text', val))
            i += 1
            continue
        if val in ('/each', '/if', 'else'):
            return nodes, i
        if val.startswith('#each '):
            body, i = parse_seq(items, i + 1)
            i += 1
            nodes.append(('each', val[6:].strip(), body))
        elif val.startswith('#if '):
            body, i = parse_seq(items, i + 1)
            els = []
            if i < len(items) and items[i] == ('g', 'else'):
                els, i = parse_seq(items, i + 1)
            i += 1
            nodes.append(('if', val[4:].strip(), body, els))
        else:
            nodes.append(('var', val))
            i += 1
    return nodes, i


def lookup(name, stack):
    if name == '.':
        return True, stack[-1][0]
    if name == '@index':
        for _, idx in reversed(stack):
            if idx is not None:
                return True, idx
        return False, None
    parts = name.split('.')
    for val, _ in reversed(stack):
        if isinstance(val, dict):
            cur = val
            ok = True
            for p in parts:
                if isinstance(cur, dict) and p in cur:
                    cur = cur[p]
                else:
                    ok = False
                    break
            if ok:
                return True, cur
    return False, None


def to_text(v):
    if v is None:
        return ''
    if isinstance(v, str):
        return v
    if isinstance(v, bool):
        return 'true' if v else 'false'
    if isinstance(v, (int, float)):
        return json.dumps(v)
    return json.dumps(v, separators=(',', ':'))


def truthy(v):
    if v is None or v is False:
        return False
    if v is True:
        return True
    if isinstance(v, (int, float)):
        return v != 0
    if isinstance(v, (str, list)):
        return len(v) > 0
    return True


def eval_var(expr, stack):
    parts = [p.strip() for p in expr.split('|')]
    found, v = lookup(parts[0], stack)
    for f in parts[1:]:
        if f.startswith('default:'):
            if not found or v is None or v == '':
                found = True
                v = f[len('default:'):]
        elif f == 'upper':
            if found:
                v = to_text(v).upper()
        elif f == 'lower':
            if found:
                v = to_text(v).lower()
        elif f == 'len':
            if found:
                v = len(v) if isinstance(v, (str, list)) else 0
    return to_text(v) if found else ''


def render(nodes, stack, out):
    for node in nodes:
        t = node[0]
        if t == 'text':
            out.append(node[1])
        elif t == 'var':
            out.append(eval_var(node[1], stack))
        elif t == 'each':
            found, arr = lookup(node[1], stack)
            if found and isinstance(arr, list):
                for i, el in enumerate(arr):
                    render(node[2], stack + [(el, i)], out)
        elif t == 'if':
            found, v = lookup(node[1], stack)
            if found and truthy(v):
                render(node[2], stack, out)
            else:
                render(node[3], stack, out)


def main():
    text = sys.stdin.read()
    head, _, tmpl = text.partition('\n')
    data = json.loads(head)
    nodes, _ = parse_seq(tokenize(tmpl), 0)
    out = []
    render(nodes, [(data, None)], out)
    sys.stdout.write(''.join(out))


if __name__ == '__main__':
    main()
