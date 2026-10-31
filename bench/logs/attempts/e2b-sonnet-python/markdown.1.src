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
    s = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', s)
    s = re.sub(r'\x00(\d+)\x00', lambda m: codes[int(m.group(1))], s)
    return s


lines = sys.stdin.read().split("\n")
out = []
heads = []
used = {}
i = 0
n = len(lines)
H = re.compile(r'^(#{1,6}) (.*)$')
UL = re.compile(r'^- (.*)$')
OL = re.compile(r'^\d+\. (.*)$')


def special(l):
    return l == "```" or H.match(l) or UL.match(l) or OL.match(l)


while i < n:
    l = lines[i].rstrip("\r")
    if l.strip() == "":
        i += 1
        continue
    if l == "```":
        i += 1
        buf = []
        while i < n and lines[i].rstrip("\r") != "```":
            buf.append(esc(lines[i].rstrip("\r")))
            i += 1
        i += 1
        out.append("<pre><code>" + "\n".join(buf) + "</code></pre>")
        continue
    m = H.match(l)
    if m:
        lvl = len(m.group(1))
        raw = m.group(2)
        base = re.sub(r'[^a-z0-9]+', '-', raw.lower()).strip('-')
        if base in used:
            used[base] += 1
            hid = "%s-%d" % (base, used[base])
        else:
            used[base] = 1
            hid = base
        out.append('<h%d id="%s">%s</h%d>' % (lvl, hid, inline(raw), lvl))
        heads.append((hid, esc(raw.replace("`", "").replace("*", ""))))
        i += 1
        continue
    if UL.match(l) or OL.match(l):
        rx = UL if UL.match(l) else OL
        tag = "ul" if rx is UL else "ol"
        out.append("<%s>" % tag)
        while i < n:
            mm = rx.match(lines[i].rstrip("\r"))
            if not mm:
                break
            out.append("<li>%s</li>" % inline(mm.group(1)))
            i += 1
        out.append("</%s>" % tag)
        continue
    buf = []
    while i < n:
        l = lines[i].rstrip("\r")
        if l.strip() == "" or special(l):
            break
        buf.append(l)
        i += 1
    out.append("<p>" + inline(" ".join(buf)) + "</p>")

if len(heads) >= 2:
    toc = ["<nav>"]
    for hid, t in heads:
        toc.append('<a href="#%s">%s</a>' % (hid, t))
    toc.append("</nav>")
    out = toc + out
print("\n".join(out))
