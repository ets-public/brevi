import re
import sys

HEAD = re.compile(r"^(#{1,6}) (.*)$")
OL = re.compile(r"^[0-9]+\. (.*)$")


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def inline(s):
    s = esc(s)
    codes = []

    def stash_code(m):
        codes.append(f"<code>{m.group(1)}</code>")
        return "\x00%d\x00" % (len(codes) - 1)

    s = re.sub(r"`([^`]*)`", stash_code, s)
    s = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", s)
    s = re.sub(r"\*(.+?)\*", r"<em>\1</em>", s)
    s = re.sub(r"\[([^\]]*)\]\(([^)]*)\)", r'<a href="\2">\1</a>', s)
    s = re.sub(r"\x00(\d+)\x00", lambda m: codes[int(m.group(1))], s)
    return s


def plain(s):
    return esc(s.replace("`", "").replace("*", ""))


def slugify(s):
    s = re.sub(r"[^a-z0-9]+", "-", s.lower())
    return s.strip("-")


lines = [ln.rstrip("\r\n") for ln in sys.stdin]
blocks = []   # list of lists of output lines
headings = []  # (id, plain text)
used_ids = set()


def make_id(text):
    base = slugify(text)
    cand = base
    n = 2
    while cand in used_ids:
        cand = f"{base}-{n}"
        n += 1
    used_ids.add(cand)
    return cand


def is_block_start(ln):
    return (ln == "" or ln == "```" or HEAD.match(ln) is not None
            or ln.startswith("- ") or OL.match(ln) is not None)


i = 0
n = len(lines)
while i < n:
    ln = lines[i]
    if ln.strip() == "":
        i += 1
        continue
    if ln == "```":
        body = []
        i += 1
        while i < n and lines[i] != "```":
            body.append(lines[i])
            i += 1
        i += 1  # skip closing fence
        code = "\n".join(esc(b) for b in body)
        blocks.append([f"<pre><code>{code}</code></pre>"])
        continue
    m = HEAD.match(ln)
    if m:
        level = len(m.group(1))
        text = m.group(2)
        hid = make_id(text.replace("`", "").replace("*", ""))
        headings.append((hid, plain(text)))
        blocks.append([f'<h{level} id="{hid}">{inline(text)}</h{level}>'])
        i += 1
        continue
    if ln.startswith("- ") or OL.match(ln):
        ordered = not ln.startswith("- ")
        items = []
        while i < n:
            cur = lines[i]
            if not ordered and cur.startswith("- "):
                items.append(cur[2:])
            elif ordered and OL.match(cur):
                items.append(OL.match(cur).group(1))
            else:
                break
            i += 1
        tag = "ol" if ordered else "ul"
        out = [f"<{tag}>"] + [f"<li>{inline(it)}</li>" for it in items] + [f"</{tag}>"]
        blocks.append(out)
        continue
    # paragraph
    para = []
    while i < n and not is_block_start(lines[i]):
        para.append(lines[i].strip())
        i += 1
    blocks.append([f"<p>{inline(' '.join(para))}</p>"])

out_lines = []
if len(headings) >= 2:
    out_lines.append("<nav>")
    for hid, text in headings:
        out_lines.append(f'<a href="#{hid}">{text}</a>')
    out_lines.append("</nav>")
for b in blocks:
    out_lines.extend(b)
print("\n".join(out_lines))
