import sys, re
def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
def fmt_plain(s):
    s = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", s)
    s = re.sub(r"\*(.+?)\*", r"<em>\1</em>", s)
    s = re.sub(r"\[([^\]]*)\]\(([^)]*)\)", r'<a href="\2">\1</a>', s)
    return s
def inline(s):
    s = esc(s)
    parts = re.split(r"(`[^`]+`)", s)
    out = []
    for p in parts:
        if len(p) >= 2 and p[0] == "`" and p[-1] == "`":
            out.append("<code>" + p[1:-1] + "</code>")
        else:
            out.append(fmt_plain(p))
    return "".join(out)
lines = sys.stdin.read().split("\n")
blocks = []
headings = []
used = {}
i = 0
n = len(lines)
hre = re.compile(r"^(#{1,6}) (.*)$")
ure = re.compile(r"^- (.*)$")
ore = re.compile(r"^\d+\. (.*)$")
def special(l):
    return l == "```" or hre.match(l) or ure.match(l) or ore.match(l)
while i < n:
    l = lines[i].rstrip("\r")
    if l.strip() == "":
        i += 1; continue
    if l == "```":
        i += 1; code = []
        while i < n and lines[i].rstrip("\r") != "```":
            code.append(lines[i].rstrip("\r")); i += 1
        i += 1
        blocks.append("<pre><code>" + "\n".join(esc(c) for c in code) + "</code></pre>")
        continue
    m = hre.match(l)
    if m:
        lvl = len(m.group(1)); text = m.group(2).strip()
        base = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
        if base in used:
            used[base] += 1
            hid = "%s-%d" % (base, used[base])
        else:
            used[base] = 1
            hid = base
        headings.append((hid, text))
        blocks.append('<h%d id="%s">%s</h%d>' % (lvl, hid, inline(text), lvl))
        i += 1; continue
    m = ure.match(l)
    if m or ore.match(l):
        rx = ure if m else ore
        tag = "ul" if m else "ol"
        items = []
        while i < n:
            mm = rx.match(lines[i].rstrip("\r"))
            if not mm: break
            items.append("<li>" + inline(mm.group(1)) + "</li>"); i += 1
        blocks.append("\n".join(["<%s>" % tag] + items + ["</%s>" % tag]))
        continue
    para = []
    while i < n:
        l = lines[i].rstrip("\r")
        if l.strip() == "" or special(l): break
        para.append(l.strip()); i += 1
    blocks.append("<p>" + inline(" ".join(para)) + "</p>")
out = []
if len(headings) >= 2:
    out.append("<nav>")
    for hid, t in headings:
        out.append('<a href="#%s">%s</a>' % (hid, esc(t.replace("`", "").replace("*", ""))))
    out.append("</nav>")
out.extend(blocks)
if out:
    print("\n".join(out))
