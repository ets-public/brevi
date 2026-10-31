import sys, re
def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
def inline(s):
    s = esc(s)
    codes = []
    def sc(m):
        codes.append(m.group(1)); return '\x00%d\x00' % (len(codes) - 1)
    s = re.sub(r'`([^`]*)`', sc, s)
    s = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', s)
    s = re.sub(r'\*([^*]+)\*', r'<em>\1</em>', s)
    s = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', s)
    s = re.sub(r'\x00(\d+)\x00', lambda m: '<code>%s</code>' % codes[int(m.group(1))], s)
    return s
lines = sys.stdin.read().split('\n')
out = []; heads = []; used = {}
i = 0; n = len(lines)
para = []
def flush():
    global para
    if para:
        out.append('<p>' + ' '.join(inline(x) for x in para) + '</p>')
        para = []
while i < n:
    l = lines[i]
    if l.strip() == '':
        flush(); i += 1; continue
    if l == '```':
        flush()
        j = i + 1; body = []
        while j < n and lines[j] != '```':
            body.append(lines[j]); j += 1
        out.append('<pre><code>' + '\n'.join(esc(x) for x in body) + '</code></pre>')
        i = j + 1; continue
    m = re.match(r'^(#{1,6}) (.*)$', l)
    if m:
        flush()
        raw = m.group(2)
        base = re.sub(r'[^a-z0-9]+', '-', raw.lower()).strip('-')
        id_ = base
        if id_ in used:
            used[base] += 1
            id_ = '%s-%d' % (base, used[base])
            while id_ in used:
                used[base] += 1
                id_ = '%s-%d' % (base, used[base])
            used[id_] = 1
        else:
            used[id_] = 1
        lv = len(m.group(1))
        out.append('<h%d id="%s">%s</h%d>' % (lv, id_, inline(raw), lv))
        heads.append((id_, raw))
        i += 1; continue
    if l.startswith('- ') or re.match(r'^\d+\. ', l):
        flush()
        ul = l.startswith('- ')
        out.append('<ul>' if ul else '<ol>')
        while i < n:
            l = lines[i]
            if ul and l.startswith('- '):
                out.append('<li>' + inline(l[2:]) + '</li>')
            elif not ul and re.match(r'^\d+\. ', l):
                out.append('<li>' + inline(re.sub(r'^\d+\. ', '', l)) + '</li>')
            else:
                break
            i += 1
        out.append('</ul>' if ul else '</ol>')
        continue
    para.append(l); i += 1
flush()
res = []
if len(heads) >= 2:
    res.append('<nav>')
    for id_, raw in heads:
        res.append('<a href="#%s">%s</a>' % (id_, esc(raw.replace('`', '').replace('*', ''))))
    res.append('</nav>')
res += out
print('\n'.join(res))
