import re
import sys

HEADING_RE = re.compile(r'^(#{1,6}) (.*)$')
UL_RE = re.compile(r'^- (.*)$')
OL_RE = re.compile(r'^\d+\. (.*)$')
FENCE = '```'


def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')


def inline(text):
    text = esc(text)
    parts = re.split(r'`([^`]*)`', text)
    out = []
    for i, part in enumerate(parts):
        if i % 2 == 1:
            out.append('<code>' + part + '</code>')
        else:
            part = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', part)
            part = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', part)
            part = re.sub(r'\*(.+?)\*', r'<em>\1</em>', part)
            out.append(part)
    return ''.join(out)


def plain(text):
    return esc(text.replace('`', '').replace('*', ''))


def make_id(text):
    return re.sub(r'[^a-z0-9]+', '-', text.lower()).strip('-')


def main():
    raw_lines = sys.stdin.read().split('\n')
    if raw_lines and raw_lines[-1] == '':
        raw_lines.pop()
    raw_lines = [l.rstrip('\r') for l in raw_lines]
    lines = [l.rstrip() for l in raw_lines]

    blocks = []      # list of lists of output lines
    headings = []    # (id, plain text)
    used_ids = set()

    i = 0
    n = len(lines)
    while i < n:
        line = lines[i]
        if line == '':
            i += 1
            continue

        if line == FENCE:
            code = []
            i += 1
            while i < n and lines[i] != FENCE:
                code.append(esc(raw_lines[i]))
                i += 1
            i += 1  # skip closing fence (if present)
            blocks.append(['<pre><code>' + '\n'.join(code) + '</code></pre>'])
            continue

        m = HEADING_RE.match(line)
        if m:
            level = len(m.group(1))
            text = m.group(2).strip()
            base = make_id(text)
            hid = base
            k = 2
            while hid in used_ids:
                hid = '%s-%d' % (base, k)
                k += 1
            used_ids.add(hid)
            headings.append((hid, plain(text)))
            blocks.append(['<h%d id="%s">%s</h%d>' % (level, hid, inline(text), level)])
            i += 1
            continue

        um = UL_RE.match(line)
        om = OL_RE.match(line)
        if um or om:
            tag = 'ul' if um else 'ol'
            rx = UL_RE if um else OL_RE
            items = []
            while i < n:
                mm = rx.match(lines[i])
                if not mm:
                    break
                items.append('<li>' + inline(mm.group(1)) + '</li>')
                i += 1
            blocks.append(['<%s>' % tag] + items + ['</%s>' % tag])
            continue

        para = []
        while i < n:
            l = lines[i]
            if l == '' or l == FENCE or HEADING_RE.match(l) or UL_RE.match(l) or OL_RE.match(l):
                break
            para.append(l)
            i += 1
        blocks.append(['<p>' + inline(' '.join(para)) + '</p>'])

    out = []
    if len(headings) >= 2:
        out.append('<nav>')
        for hid, txt in headings:
            out.append('<a href="#%s">%s</a>' % (hid, txt))
        out.append('</nav>')
    for b in blocks:
        out.extend(b)
    if out:
        sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
