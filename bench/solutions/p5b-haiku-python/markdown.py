import html
import re
import sys

HEADING_RE = re.compile(r"^(#{1,6}) (.*)$")
UL_RE = re.compile(r"^- (.*)$")
OL_RE = re.compile(r"^[0-9]+\. (.*)$")
FENCE = "```"


def slugify(text):
    s = re.sub(r"[^a-z0-9]+", "-", text.lower())
    return s.strip("-")


def fmt(s):
    s = re.sub(r"\[([^\]]*)\]\(([^)]*)\)", r'<a href="\2">\1</a>', s)
    s = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", s)
    s = re.sub(r"\*(.+?)\*", r"<em>\1</em>", s)
    return s


def inline(text):
    s = html.escape(text, quote=False)
    parts = re.split(r"`([^`]*)`", s)
    out = []
    for i, p in enumerate(parts):
        if i % 2 == 1:
            out.append("<code>" + p + "</code>")
        else:
            out.append(fmt(p))
    return "".join(out)


def is_block_start(line):
    return (
        line == FENCE
        or HEADING_RE.match(line) is not None
        or UL_RE.match(line) is not None
        or OL_RE.match(line) is not None
    )


def main():
    lines = sys.stdin.read().splitlines()
    n = len(lines)
    blocks = []  # list of (kind, text) ; kind 'heading' keeps (level, id, raw)
    headings = []
    used = set()

    i = 0
    while i < n:
        line = lines[i]
        if line.strip() == "":
            i += 1
            continue

        if line == FENCE:
            j = i + 1
            code = []
            while j < n and lines[j] != FENCE:
                code.append(html.escape(lines[j], quote=False))
                j += 1
            blocks.append("<pre><code>" + "\n".join(code) + "</code></pre>")
            i = j + 1
            continue

        m = HEADING_RE.match(line)
        if m:
            level = len(m.group(1))
            raw = m.group(2).strip()
            base = slugify(raw)
            hid = base
            k = 2
            while hid in used:
                hid = f"{base}-{k}"
                k += 1
            used.add(hid)
            headings.append((raw, hid))
            blocks.append(f'<h{level} id="{hid}">{inline(raw)}</h{level}>')
            i += 1
            continue

        ul = UL_RE.match(line)
        ol = OL_RE.match(line) if not ul else None
        if ul or ol:
            rx = UL_RE if ul else OL_RE
            tag = "ul" if ul else "ol"
            items = []
            while i < n:
                mm = rx.match(lines[i])
                if not mm:
                    break
                items.append("<li>" + inline(mm.group(1)) + "</li>")
                i += 1
            blocks.append("\n".join([f"<{tag}>"] + items + [f"</{tag}>"]))
            continue

        para = []
        while i < n and lines[i].strip() != "" and not is_block_start(lines[i]):
            para.append(lines[i].strip())
            i += 1
        blocks.append("<p>" + inline(" ".join(para)) + "</p>")

    out = []
    if len(headings) >= 2:
        out.append("<nav>")
        for raw, hid in headings:
            plain = html.escape(raw.replace("`", "").replace("*", ""), quote=False)
            out.append(f'<a href="#{hid}">{plain}</a>')
        out.append("</nav>")
    out.extend(blocks)
    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
