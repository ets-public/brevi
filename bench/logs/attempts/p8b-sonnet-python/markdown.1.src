import sys, re

def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

def inline(s):
    s = esc(s)
    codes = []
    def sc(m):
        codes.append(m.group(1))
        return "\x00%d\x00" % (len(codes) - 1)
    s = re.sub(r'`([^`]*)`', sc, s)
    s = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', s)
    s = re.sub(r'\*(.+?)\*', r'<em>\1</em>', s)
    s = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', s)
    s = re.sub(r'\x00(\d+)\x00', lambda m: "<code>%s</code>" % codes[int(m.group(1))], s)
    return s

lines = sys.stdin.read().split("\n")
blocks = []
heads = []
used = {}
i = 0
n = len(lines)
hre = re.compile(r'^(#{1,6}) (.*)$')
ulre = re.compile(r'^- (.*)$')
olre = re.compile(r'^\d+\. (.*)$')

def special(l):
    return l.strip() == "" or l == "```" or hre.match(l) or ulre.match(l) or olre.match(l)

while i < n:
    l = lines[i].rstrip("\r")
    if l.strip() == "":
        i += 1
        continue
    if l == "```":
        j = i + 1
        body = []
        while j < n and lines[j].rstrip("\r") != "```":
            body.append(esc(lines[j].rstrip("\r")))
            j += 1
        blocks.append("<pre><code>" + "\n".join(body) + "</code></pre>")
        i = j + 1
        continue
    m = hre.match(l)
    if m:
        lvl = len(m.group(1))
        text = m.group(2).strip()
        base = re.sub(r'[^a-z0-9]+', '-', text.lower()).strip('-')
        if base in used:
            used[base] += 1
            hid = "%s-%d" % (base, used[base])
        else:
            used[base] = 1
            hid = base
        heads.append((hid, text))
        blocks.append('<h%d id="%s">%s</h%d>' % (lvl, hid, inline(text), lvl))
        i += 1
        continue
    m = ulre.match(l)
    if m:
        items = []
        while i < n:
            mm = ulre.match(lines[i].rstrip("\r"))
            if not mm:
                break
            items.append("<li>%s</li>" % inline(mm.group(1)))
            i += 1
        blocks.append("<ul>\n" + "\n".join(items) + "\n</ul>")
        continue
    m = olre.match(l)
    if m:
        items = []
        while i < n:
            mm = olre.match(lines[i].rstrip("\r"))
            if not mm:
                break
            items.append("<li>%s</li>" % inline(mm.group(1)))
            i += 1
        blocks.append("<ol>\n" + "\n".join(items) + "\n</ol>")
        continue
    para = []
    while i < n and not special(lines[i].rstrip("\r")):
        para.append(lines[i].rstrip("\r"))
        i += 1
    blocks.append("<p>" + inline(" ".join(para)) + "</p>")

out = []
if len(heads) >= 2:
    out.append("<nav>")
    for hid, text in heads:
        out.append('<a href="#%s">%s</a>' % (hid, esc(text.replace("`", "").replace("*", ""))))
    out.append("</nav>")
out.extend(blocks)
if out:
    print("\n".join(out))
