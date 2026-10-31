import sys, re

def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')

def fmt(s):
    s = esc(s)
    parts = re.split(r'(`[^`]*`)', s)
    out = []
    for p in parts:
        if len(p) >= 2 and p.startswith('`') and p.endswith('`'):
            out.append('<code>' + p[1:-1] + '</code>')
        else:
            p = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', p)
            p = re.sub(r'\*(.+?)\*', r'<em>\1</em>', p)
            p = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', p)
            out.append(p)
    return ''.join(out)

lines = sys.stdin.read().split('\n')
blocks = []
headings = []
used = {}
i = 0
n = len(lines)
HRE = re.compile(r'^(#{1,6}) (.*)$')
OL = re.compile(r'^\d+\. (.*)$')
para = []

def flush():
    global para
    if para:
        blocks.append('<p>' + ' '.join(fmt(x) for x in para) + '</p>')
        para = []

while i < n:
    l = lines[i]
    if l.strip() == '':
        flush(); i += 1; continue
    if l == '```':
        flush()
        j = i + 1
        code = []
        while j < n and lines[j] != '```':
            code.append(lines[j]); j += 1
        blocks.append('<pre><code>' + '\n'.join(esc(c) for c in code) + '</code></pre>')
        i = j + 1
        continue
    m = HRE.match(l)
    if m:
        flush()
        lvl = len(m.group(1)); raw = m.group(2).strip()
        base = re.sub(r'[^a-z0-9]+', '-', raw.lower()).strip('-')
        if base in used:
            used[base] += 1
            hid = base + '-' + str(used[base])
        else:
            used[base] = 1
            hid = base
        headings.append((hid, raw))
        blocks.append('<h%d id="%s">%s</h%d>' % (lvl, hid, fmt(raw), lvl))
        i += 1; continue
    if l.startswith('- '):
        flush()
        items = []
        while i < n and lines[i].startswith('- '):
            items.append(lines[i][2:]); i += 1
        blocks.append('<ul>\n' + '\n'.join('<li>' + fmt(x) + '</li>' for x in items) + '\n</ul>')
        continue
    if OL.match(l):
        flush()
        items = []
        while i < n and OL.match(lines[i]):
            items.append(OL.match(lines[i]).group(1)); i += 1
        blocks.append('<ol>\n' + '\n'.join('<li>' + fmt(x) + '</li>' for x in items) + '\n</ol>')
        continue
    para.append(l)
    i += 1
flush()

out = []
if len(headings) >= 2:
    out.append('<nav>')
    for hid, raw in headings:
        out.append('<a href="#%s">%s</a>' % (hid, esc(raw.replace('`', '').replace('*', ''))))
    out.append('</nav>')
out.extend(blocks)
print('\n'.join(out))
