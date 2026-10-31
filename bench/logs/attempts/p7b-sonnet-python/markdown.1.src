import sys, re
def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
def inline(s):
    s = esc(s)
    codes = []
    def sv(m):
        codes.append(m.group(1)); return '\x00%d\x00' % (len(codes) - 1)
    s = re.sub(r'`([^`]*)`', sv, s)
    s = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', s)
    s = re.sub(r'\*(.+?)\*', r'<em>\1</em>', s)
    s = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', s)
    s = re.sub(r'\x00(\d+)\x00', lambda m: '<code>' + codes[int(m.group(1))] + '</code>', s)
    return s
lines = sys.stdin.read().split('\n')
if lines and lines[-1] == '': lines.pop()
out = []; heads = []; used = {}
H = re.compile(r'^(#{1,6}) (.*)$')
UL = re.compile(r'^- (.*)$')
OL = re.compile(r'^\d+\. (.*)$')
i = 0; n = len(lines)
def special(l):
    return l.strip() == '' or l.rstrip() == '```' or H.match(l) or UL.match(l) or OL.match(l)
while i < n:
    l = lines[i]
    if l.strip() == '':
        i += 1; continue
    if l.rstrip() == '```':
        i += 1; buf = []
        while i < n and lines[i].rstrip() != '```':
            buf.append(esc(lines[i])); i += 1
        i += 1
        out.append('<pre><code>' + '\n'.join(buf) + '</code></pre>')
        continue
    m = H.match(l)
    if m:
        lv = len(m.group(1)); t = m.group(2).strip()
        base = re.sub(r'[^a-z0-9]+', '-', t.lower()).strip('-')
        id_ = base
        if id_ in used:
            used[base] += 1
            id_ = '%s-%d' % (base, used[base])
            while id_ in used:
                used[base] += 1; id_ = '%s-%d' % (base, used[base])
            used[id_] = 1
        else:
            used[id_] = 1
        heads.append((id_, t))
        out.append('<h%d id="%s">%s</h%d>' % (lv, id_, inline(t), lv))
        i += 1; continue
    m = UL.match(l) or OL.match(l)
    if m:
        rx = UL if UL.match(l) else OL
        tag = 'ul' if rx is UL else 'ol'
        out.append('<%s>' % tag)
        while i < n and rx.match(lines[i]):
            out.append('<li>%s</li>' % inline(rx.match(lines[i]).group(1))); i += 1
        out.append('</%s>' % tag)
        continue
    buf = []
    while i < n and not special(lines[i]):
        buf.append(lines[i].strip()); i += 1
    out.append('<p>' + inline(' '.join(buf)) + '</p>')
if len(heads) >= 2:
    print('<nav>')
    for id_, t in heads:
        print('<a href="#%s">%s</a>' % (id_, esc(t.replace('`', '').replace('*', ''))))
    print('</nav>')
for o in out:
    print(o)
