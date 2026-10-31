import sys, re
def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
def inline(s):
    s = esc(s)
    codes = []
    def sc(m):
        codes.append(m.group(1)); return "\x00%d\x00" % (len(codes) - 1)
    s = re.sub(r"`([^`]*)`", sc, s)
    s = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", s)
    s = re.sub(r"\*(.+?)\*", r"<em>\1</em>", s)
    s = re.sub(r"\[([^\]]*)\]\(([^)]*)\)", r'<a href="\2">\1</a>', s)
    s = re.sub(r"\x00(\d+)\x00", lambda m: "<code>%s</code>" % codes[int(m.group(1))], s)
    return s
lines = sys.stdin.read().split("\n")
out = []
heads = []
used = {}
HRE = re.compile(r"^(#{1,6}) (.*)$")
OL = re.compile(r"^\d+\. (.*)$")
i = 0
n = len(lines)
def special(l):
    return HRE.match(l) or l.startswith("- ") or OL.match(l) or l == "```"
while i < n:
    l = lines[i]
    if not l.strip():
        i += 1; continue
    if l == "```":
        i += 1; code = []
        while i < n and lines[i] != "```":
            code.append(esc(lines[i])); i += 1
        i += 1
        out.append("<pre><code>" + "\n".join(code) + "</code></pre>")
        continue
    m = HRE.match(l)
    if m:
        lvl = len(m.group(1)); text = m.group(2).strip()
        base = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
        if base in used:
            used[base] += 1; hid = "%s-%d" % (base, used[base])
        else:
            used[base] = 1; hid = base
        heads.append((hid, text))
        out.append('<h%d id="%s">%s</h%d>' % (lvl, hid, inline(text), lvl))
        i += 1; continue
    if l.startswith("- ") or OL.match(l):
        ul = l.startswith("- ")
        out.append("<ul>" if ul else "<ol>")
        while i < n:
            l = lines[i]
            if ul and l.startswith("- "): t = l[2:]
            elif not ul and OL.match(l): t = OL.match(l).group(1)
            else: break
            out.append("<li>%s</li>" % inline(t)); i += 1
        out.append("</ul>" if ul else "</ol>")
        continue
    para = []
    while i < n and lines[i].strip() and not special(lines[i]):
        para.append(lines[i].strip()); i += 1
    out.append("<p>" + inline(" ".join(para)) + "</p>")
if len(heads) >= 2:
    print("<nav>")
    for hid, t in heads:
        print('<a href="#%s">%s</a>' % (hid, esc(t.replace("`", "").replace("*", ""))))
    print("</nav>")
for o in out: print(o)
