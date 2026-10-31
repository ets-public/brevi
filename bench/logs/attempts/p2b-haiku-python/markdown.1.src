import re
import sys

HEADING_RE = re.compile(r"^(#{1,6}) (.*)$")
UL_RE = re.compile(r"^- (.*)$")
OL_RE = re.compile(r"^[0-9]+\. (.*)$")
LINK_RE = re.compile(r"\[([^\]]*)\]\(([^)]*)\)")
BOLD_RE = re.compile(r"\*\*(.+?)\*\*")
EM_RE = re.compile(r"\*(.+?)\*")


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def inline(text):
    text = esc(text)
    parts = text.split("`")
    out = []
    for idx, seg in enumerate(parts):
        if idx % 2 == 1:
            out.append("<code>" + seg + "</code>")
        else:
            seg = LINK_RE.sub(lambda m: '<a href="' + m.group(2) + '">' + m.group(1) + "</a>", seg)
            seg = BOLD_RE.sub(r"<strong>\1</strong>", seg)
            seg = EM_RE.sub(r"<em>\1</em>", seg)
            out.append(seg)
    return "".join(out)


def make_id(text):
    s = re.sub(r"[^a-z0-9]+", "-", text.lower())
    return s.strip("-")


def main():
    lines = sys.stdin.read().splitlines()
    blocks = []
    i = 0
    n = len(lines)
    while i < n:
        line = lines[i]
        if line.strip() == "":
            i += 1
            continue
        if line == "```":
            j = i + 1
            code = []
            while j < n and lines[j] != "```":
                code.append(lines[j])
                j += 1
            blocks.append(("code", code))
            i = j + 1
            continue
        m = HEADING_RE.match(line)
        if m:
            blocks.append(("h", len(m.group(1)), m.group(2).strip()))
            i += 1
            continue
        if UL_RE.match(line) or OL_RE.match(line):
            ordered = not UL_RE.match(line)
            rx = OL_RE if ordered else UL_RE
            items = []
            while i < n and rx.match(lines[i]):
                items.append(rx.match(lines[i]).group(1).strip())
                i += 1
            blocks.append(("list", ordered, items))
            continue
        para = []
        while i < n:
            cur = lines[i]
            if (cur.strip() == "" or cur == "```" or HEADING_RE.match(cur)
                    or UL_RE.match(cur) or OL_RE.match(cur)):
                break
            para.append(cur.strip())
            i += 1
        blocks.append(("p", para))

    # assign heading ids
    used = set()
    heads = []
    for b in blocks:
        if b[0] == "h":
            base = make_id(b[2])
            hid = base
            if base in used:
                k = 2
                while f"{base}-{k}" in used:
                    k += 1
                hid = f"{base}-{k}"
            used.add(hid)
            heads.append((hid, b[2]))

    out = []
    if len(heads) >= 2:
        out.append("<nav>")
        for hid, text in heads:
            plain = text.replace("`", "").replace("*", "")
            out.append(f'<a href="#{hid}">{esc(plain)}</a>')
        out.append("</nav>")

    hi = 0
    for b in blocks:
        if b[0] == "h":
            hid = heads[hi][0]
            hi += 1
            out.append(f'<h{b[1]} id="{hid}">{inline(b[2])}</h{b[1]}>')
        elif b[0] == "code":
            body = "\n".join(esc(x) for x in b[1])
            out.append("<pre><code>" + body + "</code></pre>")
        elif b[0] == "list":
            tag = "ol" if b[1] else "ul"
            out.append(f"<{tag}>")
            for item in b[2]:
                out.append(f"<li>{inline(item)}</li>")
            out.append(f"</{tag}>")
        else:
            out.append("<p>" + inline(" ".join(b[1])) + "</p>")

    sys.stdout.write("\n".join(out) + ("\n" if out else ""))


if __name__ == "__main__":
    main()
