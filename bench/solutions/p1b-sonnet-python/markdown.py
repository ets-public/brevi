import sys, re


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def inline(s):
    s = esc(s)
    codes = []

    def keep(m):
        codes.append("<code>" + m.group(1) + "</code>")
        return "\x00%d\x00" % (len(codes) - 1)

    s = re.sub(r'`([^`]*)`', keep, s)
    s = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', s)
    s = re.sub(r'\*(.+?)\*', r'<em>\1</em>', s)
    s = re.sub(r'\[([^\]]+)\]\(([^)]+)\)', r'<a href="\2">\1</a>', s)
    s = re.sub(r'\x00(\d+)\x00', lambda m: codes[int(m.group(1))], s)
    return s


lines = sys.stdin.read().split("\n")
blocks = []
headings = []
used = set()
i = 0
n = len(lines)
hre = re.compile(r'^(#{1,6}) (.*)$')
ore = re.compile(r'^\d+\. (.*)$')
while i < n:
    l = lines[i]
    if l.strip() == "":
        i += 1
        continue
    if l == "```":
        j = i + 1
        body = []
        while j < n and lines[j] != "```":
            body.append(lines[j])
            j += 1
        blocks.append("<pre><code>" + "\n".join(esc(x) for x in body) + "</code></pre>")
        i = j + 1
        continue
    m = hre.match(l)
    if m:
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
    if l.startswith("- "):
        items = []
        while i < n and lines[i].startswith("- "):
            items.append("<li>" + inline(lines[i][2:]) + "</li>")
            i += 1
        blocks.append("<ul>\n" + "\n".join(items) + "\n</ul>")
        continue
    if ore.match(l):
        items = []
        while i < n and ore.match(lines[i]):
            items.append("<li>" + inline(ore.match(lines[i]).group(1)) + "</li>")
            i += 1
        blocks.append("<ol>\n" + "\n".join(items) + "\n</ol>")
        continue
    para = []
    while i < n and lines[i].strip() != "" and lines[i] != "```" and not hre.match(lines[i]) \
            and not lines[i].startswith("- ") and not ore.match(lines[i]):
        para.append(lines[i])
        i += 1
    blocks.append("<p>" + " ".join(para) + "</p>" if False else "<p>" + inline(" ".join(para)) + "</p>")

out = []
if len(headings) >= 2:
    out.append("<nav>")
    for hid, text in headings:
        out.append('<a href="#%s">%s</a>' % (hid, esc(text.replace("`", "").replace("*", ""))))
    out.append("</nav>")
out.extend(blocks)
print("\n".join(out))
