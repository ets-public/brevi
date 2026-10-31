import re
import sys

FENCE = '```'
HEAD_RE = re.compile(r'(#{1,6}) (.*)')
UL_RE = re.compile(r'- (.*)')
OL_RE = re.compile(r'[0-9]+\. (.*)')


def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')


def inline(text):
    s = esc(text)
    store = []

    def keep(m):
        store.append('<code>' + m.group(1) + '</code>')
        return '\x00' + str(len(store) - 1) + '\x00'

    s = re.sub(r'`([^`]*)`', keep, s)
    s = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', s)
    s = re.sub(r'\*(.+?)\*', r'<em>\1</em>', s)
    s = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', s)
    s = re.sub(r'\x00([0-9]+)\x00', lambda m: store[int(m.group(1))], s)
    return s


def slug(text):
    return re.sub(r'[^a-z0-9]+', '-', text.lower()).strip('-')


def list_kind(line):
    if line.startswith('- '):
        return 'ul'
    if OL_RE.fullmatch(line):
        return 'ol'
    return None


def list_text(line):
    if line.startswith('- '):
        return line[2:].strip()
    return OL_RE.fullmatch(line).group(1).strip()


def is_block_start(line):
    return (line.rstrip() == FENCE or HEAD_RE.fullmatch(line) is not None
            or list_kind(line) is not None)


def main():
    lines = sys.stdin.read().split('\n')
    if lines and lines[-1] == '':
        lines.pop()
    lines = [l.rstrip('\r') for l in lines]
    n = len(lines)
    blocks = []
    headings = []
    used = set()
    i = 0
    while i < n:
        line = lines[i]
        if line.strip() == '':
            i += 1
            continue
        if line.rstrip() == FENCE:
            j = i + 1
            code = []
            while j < n and lines[j].rstrip() != FENCE:
                code.append(lines[j])
                j += 1
            blocks.append('<pre><code>' + esc('\n'.join(code)) + '</code></pre>')
            i = j + 1
            continue
        m = HEAD_RE.fullmatch(line)
        if m:
            level = len(m.group(1))
            raw = m.group(2).strip()
            base = slug(raw)
            hid = base
            k = 2
            while hid in used:
                hid = f'{base}-{k}'
                k += 1
            used.add(hid)
            headings.append((hid, raw))
            blocks.append(f'<h{level} id="{hid}">{inline(raw)}</h{level}>')
            i += 1
            continue
        kind = list_kind(line)
        if kind:
            items = []
            while i < n and list_kind(lines[i]) == kind:
                items.append(list_text(lines[i]))
                i += 1
            body = [f'<{kind}>'] + [f'<li>{inline(t)}</li>' for t in items] + [f'</{kind}>']
            blocks.append('\n'.join(body))
            continue
        para = []
        while i < n and lines[i].strip() != '' and not is_block_start(lines[i]):
            para.append(lines[i].strip())
            i += 1
        blocks.append('<p>' + inline(' '.join(para)) + '</p>')

    out = []
    if len(headings) >= 2:
        out.append('<nav>')
        for hid, raw in headings:
            text = raw.replace('`', '').replace('*', '')
            out.append(f'<a href="#{hid}">{esc(text)}</a>')
        out.append('</nav>')
    out.extend(blocks)
    if out:
        sys.stdout.write('\n'.join(out) + '\n')


main()
