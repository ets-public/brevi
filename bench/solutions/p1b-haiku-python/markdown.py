import re
import sys

HEADING = re.compile(r"^(#{1,6}) (.*)$")
UL_ITEM = re.compile(r"^- (.*)$")
OL_ITEM = re.compile(r"^\d+\. (.*)$")


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def inline_plain(s):
    # s is already HTML-escaped; code spans are split out first
    parts = re.split(r"`([^`]*)`", s)
    out = []
    for i, part in enumerate(parts):
        if i % 2 == 1:
            out.append(f"<code>{part}</code>")
        else:
            part = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", part)
            part = re.sub(r"\*(.+?)\*", r"<em>\1</em>", part)
            part = re.sub(r"\[([^\]]*)\]\(([^)]*)\)", r'<a href="\2">\1</a>', part)
            out.append(part)
    return "".join(out)


def inline(s):
    return inline_plain(esc(s))


def heading_id(text):
    s = re.sub(r"[^a-z0-9]+", "-", text.lower())
    return s.strip("-")


def is_block_start(line):
    return (
        line == "```"
        or HEADING.match(line) is not None
        or UL_ITEM.match(line) is not None
        or OL_ITEM.match(line) is not None
    )


def main():
    lines = [l.rstrip("\r\n") for l in sys.stdin.read().split("\n")]
    if lines and lines[-1] == "":
        lines.pop()

    blocks = []  # list of lists of output lines
    headings = []  # (id, raw text)
    used = set()
    i = 0
    n = len(lines)
    while i < n:
        line = lines[i]
        if line.strip() == "":
            i += 1
            continue
        if line == "```":
            i += 1
            code = []
            while i < n and lines[i] != "```":
                code.append(esc(lines[i]))
                i += 1
            i += 1  # skip closing fence
            blocks.append([f"<pre><code>{chr(10).join(code)}</code></pre>"])
            continue
        m = HEADING.match(line)
        if m:
            level = len(m.group(1))
            raw = m.group(2).strip()
            base = heading_id(raw)
            hid = base
            k = 2
            while hid in used:
                hid = f"{base}-{k}"
                k += 1
            used.add(hid)
            headings.append((hid, raw))
            blocks.append([f'<h{level} id="{hid}">{inline(raw)}</h{level}>'])
            i += 1
            continue
        if UL_ITEM.match(line) or OL_ITEM.match(line):
            is_ul = UL_ITEM.match(line) is not None
            rx = UL_ITEM if is_ul else OL_ITEM
            tag = "ul" if is_ul else "ol"
            items = []
            while i < n:
                m = rx.match(lines[i])
                if not m:
                    break
                items.append(f"<li>{inline(m.group(1))}</li>")
                i += 1
            blocks.append([f"<{tag}>"] + items + [f"</{tag}>"])
            continue
        para = []
        while i < n and lines[i].strip() != "" and not is_block_start(lines[i]):
            para.append(lines[i].strip())
            i += 1
        blocks.append([f"<p>{inline(' '.join(para))}</p>"])

    out = []
    if len(headings) >= 2:
        out.append("<nav>")
        for hid, raw in headings:
            plain = raw.replace("`", "").replace("*", "")
            out.append(f'<a href="#{hid}">{esc(plain)}</a>')
        out.append("</nav>")
    for b in blocks:
        out.extend(b)
    sys.stdout.write("\n".join(out) + ("\n" if out else ""))


if __name__ == "__main__":
    main()
