import json
import re
import sys


def tokenize(text):
    parts = re.split(r'\{\{(.*?)\}\}', text, flags=re.S)
    toks = []
    for idx, p in enumerate(parts):
        if idx % 2 == 0:
            if p:
                toks.append(('text', p))
        else:
            toks.append(('tag', p.strip()))
    return toks


def is_tag(toks, i, name):
    return i < len(toks) and toks[i][0] == 'tag' and toks[i][1] == name


def parse(toks, i, ends):
    nodes = []
    while i < len(toks):
        kind, val = toks[i]
        if kind == 'text':
            nodes.append(('text', val))
            i += 1
            continue
        if val in ends:
            return nodes, i
        if val.startswith('#each '):
            body, i = parse(toks, i + 1, {'/each'})
            nodes.append(('each', val[6:].strip(), body))
            i += 1
        elif val.startswith('#if '):
            body, i = parse(toks, i + 1, {'else', '/if'})
            alt = []
            if is_tag(toks, i, 'else'):
                alt, i = parse(toks, i + 1, {'/if'})
            nodes.append(('if', val[4:].strip(), body, alt))
            i += 1
        else:
            nodes.append(('var', val))
            i += 1
    return nodes, i


def resolve(name, frames):
    if name == '.':
        return frames[-1][0]
    if name == '@index':
        for _, idx in reversed(frames):
            if idx is not None:
                return idx
        return None
    parts = name.split('.')
    for obj, _ in reversed(frames):
        if isinstance(obj, dict) and parts[0] in obj:
            cur = obj
            for p in parts:
                if isinstance(cur, dict) and p in cur:
                    cur = cur[p]
                else:
                    return None
            return cur
    return None


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


def eval_tag(tag, frames):
    parts = [p.strip() for p in tag.split('|')]
    v = resolve(parts[0], frames)
    for f in parts[1:]:
        name, _, arg = f.partition(':')
        name = name.strip()
        if name == 'default':
            if v is None or v == '':
                v = arg
        elif name == 'upper':
            v = to_text(v).upper()
        elif name == 'lower':
            v = to_text(v).lower()
        elif name == 'len':
            v = len(v) if isinstance(v, (str, list)) else None
    return to_text(v)


def render(nodes, frames, out):
    for node in nodes:
        kind = node[0]
        if kind == 'text':
            out.append(node[1])
        elif kind == 'var':
            out.append(eval_tag(node[1], frames))
        elif kind == 'each':
            arr = resolve(node[1], frames)
            if isinstance(arr, list):
                for idx, el in enumerate(arr):
                    render(node[2], frames + [(el, idx)], out)
        else:
            branch = node[2] if truthy(resolve(node[1], frames)) else node[3]
            render(branch, frames, out)


def main():
    text = sys.stdin.read()
    first, _, rest = text.partition('\n')
    data = json.loads(first)
    nodes, _ = parse(tokenize(rest), 0, set())
    out = []
    render(nodes, [(data, None)], out)
    sys.stdout.write(''.join(out))


main()
