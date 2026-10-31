import sys, re
def esc(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
def inline(s):
    s = esc(s)
    codes = []
    def sc(m):
        codes.append(m.group(1))
        return '\x00%d\x01' % (len(codes) - 1)
    s = re.sub(r'`([^`]*)`', sc, s)
    s = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', s)
    s = re.sub(r'\*(.+?)\*', r'<em>\1</em>', s)
    s = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', s)
    s = re.sub(r'\x00(\d+)\x01', lambda m: '<code>%s</code>' % codes[int(m.group(1))], s)
    return s
lines = sys.stdin.read().split('\n')
out = []
heads = []
used = {}
i = 0
n = len(lines)
hre = re.compile(r'(#{1,6}) (.*)')
ore = re.compile(r'\d+\. (.*)')
def special(l):
    return l == '```' or hre.fullmatch(l) or l.startswith('- ') or ore.fullmatch(l)
while i < n:
    l = lines[i]
    if not l.strip():
        i += 1
        continue
    if l == '```':
        j = i + 1
        body = []
        while j < n and lines[j] != '```':
            body.append(esc(lines[j])); j += 1
        out.append('<pre><code>' + '\n'.join(body) + '</code></pre>')
        i = j + 1
        continue
    m = hre.fullmatch(l)
    if m:
        lvl = len(m.group(1)); text = m.group(2).strip()
        base = re.sub(r'[^a-z0-9]+', '-', text.lower()).strip('-')
        hid = base
        if hid in used:
            k = used[base] + 1
            while True:
                hid = '%s-%d' % (base, k)
                if hid not in used:
                    break
                k += 1
            used[base] = k
        used[hid] = 1 if hid == base else used.get(hid, 1)
        heads.append((hid, text))
        out.append('<h%d id="%s">%s</h%d>' % (lvl, hid, inline(text), lvl))
        i += 1
        continue
    if l.startswith('- '):
        out.append('<ul>')
        while i < n and lines[i].startswith('- '):
            out.append('<li>%s</li>' % inline(lines[i][2:])); i += 1
        out.append('</ul>')
        continue
    if ore.fullmatch(l):
        out.append('<ol>')
        while i < n and ore.fullmatch(lines[i]):
            out.append('<li>%s</li>' % inline(ore.fullmatch(lines[i]).group(1))); i += 1
        out.append('</ol>')
        continue
    par = []
    while i < n and lines[i].strip() and not special(lines[i]):
        par.append(lines[i]); i += 1
    out.append('<p>' + ' '.join(par) + '</p>' if False else '<p>' + inline(' '.join(par)) + '</p>')
if len(heads) >= 2:
    print('<nav>')
    for hid, t in heads:
        print('<a href="#%s">%s</a>' % (hid, esc(t.replace('`', '').replace('*', ''))))
    print('</nav>')
for o in out:
    print(o)
