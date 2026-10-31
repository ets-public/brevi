import sys, re

def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

def fmt_plain(s):
    s = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', s)
    s = re.sub(r'\*(.+?)\*', r'<em>\1</em>', s)
    s = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', s)
    return s

def inline(s):
    s = esc(s)
    parts = re.split(r'(`[^`]+`)', s)
    out = []
    for p in parts:
        if len(p) >= 2 and p[0] == '`' and p[-1] == '`' and p.count('`') == 2:
            out.append("<code>" + p[1:-1] + "</code>")
        else:
            out.append(fmt_plain(p))
    return "".join(out)

lines = sys.stdin.read().split('\n')
blocks = []
used = set()
headings = []
i = 0
n = len(lines)
para = []
lst = None  # (tag, items)

def flush():
    global para, lst
    if para:
        blocks.append("<p>" + " ".join(inline(x) for x in para) + "</p>")
        para = []
    if lst:
        blocks.append("<%s>" % lst[0])
        for it in lst[1]:
            blocks.append("<li>" + inline(it) + "</li>")
        blocks.append("</%s>" % lst[0])
        lst = None

while i < n:
    line = lines[i].rstrip('\r')
    if line == "```":
        flush()
        j = i + 1
        code = []
        while j < n and lines[j].rstrip('\r') != "```":
            code.append(esc(lines[j].rstrip('\r')))
            j += 1
        blocks.append("<pre><code>" + "\n".join(code) + "</code></pre>")
        i = j + 1
        continue
    if not line.strip():
        flush()
        i += 1
        continue
    m = re.match(r'^(#{1,6}) (.*)$', line)
    if m:
        flush()
        lvl = len(m.group(1))
        text = m.group(2).strip()
        base = re.sub(r'[^a-z0-9]+', '-', text.lower()).strip('-')
        hid = base
        k = 2
        while hid in used:
            hid = "%s-%d" % (base, k)
            k += 1
        used.add(hid)
        headings.append((hid, text))
        blocks.append('<h%d id="%s">%s</h%d>' % (lvl, hid, inline(text), lvl))
        i += 1
        continue
    m = re.match(r'^- (.*)$', line)
    if m:
        if para or (lst and lst[0] != "ul"):
            flush()
        if not lst:
            lst = ("ul", [])
        lst[1].append(m.group(1))
        i += 1
        continue
    m = re.match(r'^\d+\. (.*)$', line)
    if m:
        if para or (lst and lst[0] != "ol"):
            flush()
        if not lst:
            lst = ("ol", [])
        lst[1].append(m.group(1))
        i += 1
        continue
    if lst:
        flush()
    para.append(line)
    i += 1
flush()
if len(headings) >= 2:
    print("<nav>")
    for hid, t in headings:
        print('<a href="#%s">%s</a>' % (hid, esc(t.replace("`", "").replace("*", ""))))
    print("</nav>")
for b in blocks:
    print(b)
