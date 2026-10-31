import sys
import re

HEAD = re.compile(r'^(#{1,6}) (.*)$')
OL = re.compile(r'^\d+\. (.*)$')


def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')


def slug(text):
    return re.sub(r'[^a-z0-9]+', '-', text.lower()).strip('-')


def inline(s):
    s = esc(s)
    codes = []

    def code_sub(m):
        codes.append(m.group(1))
        return f"\x00{len(codes) - 1}\x00"

    s = re.sub(r'`([^`]*)`', code_sub, s)
    s = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', s)
    s = re.sub(r'\*(.+?)\*', r'<em>\1</em>', s)
    s = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', s)
    s = re.sub(r'\x00(\d+)\x00', lambda m: f"<code>{codes[int(m.group(1))]}</code>", s)
    return s


def plain(text):
    return esc(text.replace('`', '').replace('*', ''))


def parse(lines):
    blocks = []
    i = 0
    n = len(lines)
    while i < n:
        line = lines[i]
        if line.strip() == '':
            i += 1
            continue
        if line.strip() == '```':
            code = []
            i += 1
            while i < n and lines[i].strip() != '```':
                code.append(lines[i])
                i += 1
            i += 1
            blocks.append(('code', code))
            continue
        m = HEAD.match(line)
        if m:
            blocks.append(('h', len(m.group(1)), m.group(2).strip()))
            i += 1
            continue
        if line.startswith('- '):
            items = []
            while i < n and lines[i].startswith('- '):
                items.append(lines[i][2:].strip())
                i += 1
            blocks.append(('ul', items))
            continue
        if OL.match(line):
            items = []
            while i < n and OL.match(lines[i]):
                items.append(OL.match(lines[i]).group(1).strip())
                i += 1
            blocks.append(('ol', items))
            continue
        para = []
        while i < n and lines[i].strip() != '' and not (
            lines[i].strip() == '```' or HEAD.match(lines[i])
            or lines[i].startswith('- ') or OL.match(lines[i])
        ):
            para.append(lines[i].strip())
            i += 1
        blocks.append(('p', para))
    return blocks


def main():
    blocks = parse(sys.stdin.read().splitlines())
    used = set()
    toc = []
    out = []
    for b in blocks:
        kind = b[0]
        if kind == 'h':
            _, level, text = b
            base = slug(text)
            hid = base
            k = 2
            while hid in used:
                hid = f"{base}-{k}"
                k += 1
            used.add(hid)
            toc.append(f'<a href="#{hid}">{plain(text)}</a>')
            out.append(f'<h{level} id="{hid}">{inline(text)}</h{level}>')
        elif kind == 'ul' or kind == 'ol':
            parts = [f'<{kind}>']
            parts += [f'<li>{inline(it)}</li>' for it in b[1]]
            parts.append(f'</{kind}>')
            out.append('\n'.join(parts))
        elif kind == 'code':
            body = '\n'.join(esc(l) for l in b[1])
            out.append(f'<pre><code>{body}</code></pre>')
        else:
            out.append('<p>' + inline(' '.join(b[1])) + '</p>')

    result = []
    if len(toc) >= 2:
        result.append('<nav>')
        result.extend(toc)
        result.append('</nav>')
    result.extend(out)
    print('\n'.join(result))


main()
