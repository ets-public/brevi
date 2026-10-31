import sys
import re
import html

HEAD = re.compile(r'^(#{1,6}) (.*)$')
UL = re.compile(r'^- (.*)$')
OL = re.compile(r'^[0-9]+\. (.*)$')
LINK = re.compile(r'\[([^\]]*)\]\(([^)]*)\)')
BOLD = re.compile(r'\*\*(.+?)\*\*')
EM = re.compile(r'\*([^*]+?)\*')
CODE = re.compile(r'(`[^`]*`)')


def esc(s):
    return html.escape(s, quote=False)


def inline(s):
    s = esc(s)
    parts = CODE.split(s)
    out = []
    for i, p in enumerate(parts):
        if i % 2 == 1:
            out.append('<code>' + p[1:-1] + '</code>')
        else:
            p = LINK.sub(r'<a href="\2">\1</a>', p)
            p = BOLD.sub(r'<strong>\1</strong>', p)
            p = EM.sub(r'<em>\1</em>', p)
            out.append(p)
    return ''.join(out)


def slug(t):
    return re.sub(r'[^a-z0-9]+', '-', t.lower()).strip('-')


def main():
    lines = [l.rstrip('\r') for l in sys.stdin.read().split('\n')]
    n = len(lines)
    blocks = []
    heads = []
    used = set()
    i = 0
    while i < n:
        line = lines[i]
        if line.strip() == '':
            i += 1
            continue
        if line.strip() == '```':
            i += 1
            code = []
            while i < n and lines[i].strip() != '```':
                code.append(esc(lines[i]))
                i += 1
            i += 1
            blocks.append('<pre><code>' + '\n'.join(code) + '</code></pre>')
            continue
        m = HEAD.match(line)
        if m:
            level = len(m.group(1))
            text = m.group(2).strip()
            base = slug(text)
            cand = base
            k = 2
            while cand in used:
                cand = f'{base}-{k}'
                k += 1
            used.add(cand)
            heads.append((cand, esc(text.replace('`', '').replace('*', ''))))
            blocks.append(f'<h{level} id="{cand}">{inline(text)}</h{level}>')
            i += 1
            continue
        if UL.match(line) or OL.match(line):
            is_ol = not UL.match(line)
            pat = OL if is_ol else UL
            items = []
            while i < n:
                m = pat.match(lines[i])
                if not m:
                    break
                items.append(f'<li>{inline(m.group(1).strip())}</li>')
                i += 1
            tag = 'ol' if is_ol else 'ul'
            blocks.append('\n'.join([f'<{tag}>'] + items + [f'</{tag}>']))
            continue
        para = []
        while i < n:
            l = lines[i]
            s = l.strip()
            if s == '' or s == '```' or HEAD.match(l) or UL.match(l) or OL.match(l):
                break
            para.append(s)
            i += 1
        blocks.append('<p>' + inline(' '.join(para)) + '</p>')

    out = []
    if len(heads) >= 2:
        out.append('<nav>')
        for hid, t in heads:
            out.append(f'<a href="#{hid}">{t}</a>')
        out.append('</nav>')
    out.extend(blocks)
    sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
