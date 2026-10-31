import re
import sys

HEADING_RE = re.compile(r"^(#{1,6}) (.*)$")
UL_RE = re.compile(r"^- (.*)$")
OL_RE = re.compile(r"^\d+\. (.*)$")


def escape(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def inline(text):
    text = escape(text)
    codes = []

    def stash_code(m):
        codes.append(m.group(1))
        return "\x00%d\x00" % (len(codes) - 1)

    text = re.sub(r"`([^`]*)`", stash_code, text)
    text = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"\*(.+?)\*", r"<em>\1</em>", text)
    text = re.sub(r"\[([^\]]*)\]\(([^)]*)\)", r'<a href="\2">\1</a>', text)
    text = re.sub(r"\x00(\d+)\x00", lambda m: "<code>%s</code>" % codes[int(m.group(1))], text)
    return text


def make_id(raw):
    s = re.sub(r"[^a-z0-9]+", "-", raw.lower())
    return s.strip("-")


def main():
    lines = [l.rstrip("\r\n") for l in sys.stdin]
    blocks = []  # list of lists of output lines
    headings = []  # (id, raw_text)
    used = set()

    i = 0
    n = len(lines)
    while i < n:
        line = lines[i]
        if line.strip() == "":
            i += 1
            continue
        if line.strip() == "```":
            code = []
            i += 1
            while i < n and lines[i].strip() != "```":
                code.append(lines[i])
                i += 1
            i += 1  # skip closing fence (or run past end)
            blocks.append(["<pre><code>" + "\n".join(escape(c) for c in code) + "</code></pre>"])
            continue
        m = HEADING_RE.match(line)
        if m:
            level = len(m.group(1))
            raw = m.group(2).strip()
            base = make_id(raw)
            hid = base
            k = 2
            while hid in used:
                hid = "%s-%d" % (base, k)
                k += 1
            used.add(hid)
            headings.append((hid, raw))
            blocks.append(["<h%d id=\"%s\">%s</h%d>" % (level, hid, inline(raw), level)])
            i += 1
            continue
        if UL_RE.match(line) or OL_RE.match(line):
            is_ol = bool(OL_RE.match(line)) and not UL_RE.match(line)
            pat = OL_RE if is_ol else UL_RE
            tag = "ol" if is_ol else "ul"
            out = ["<%s>" % tag]
            while i < n:
                mm = pat.match(lines[i])
                if not mm:
                    break
                out.append("<li>%s</li>" % inline(mm.group(1).strip()))
                i += 1
            out.append("</%s>" % tag)
            blocks.append(out)
            continue
        # Paragraph: consecutive lines that are not blank or other block starts.
        para = []
        while i < n:
            l = lines[i]
            if (l.strip() == "" or l.strip() == "```" or HEADING_RE.match(l)
                    or UL_RE.match(l) or OL_RE.match(l)):
                break
            para.append(l.strip())
            i += 1
        blocks.append(["<p>%s</p>" % inline(" ".join(para))])

    out_lines = []
    if len(headings) >= 2:
        out_lines.append("<nav>")
        for hid, raw in headings:
            plain = raw.replace("`", "").replace("*", "")
            out_lines.append('<a href="#%s">%s</a>' % (hid, escape(plain)))
        out_lines.append("</nav>")
    for b in blocks:
        out_lines.extend(b)
    if out_lines:
        sys.stdout.write("\n".join(out_lines) + "\n")


if __name__ == "__main__":
    main()
