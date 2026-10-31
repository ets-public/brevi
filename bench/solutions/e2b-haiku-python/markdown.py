import re
import sys

HEADING_RE = re.compile(r'^(#{1,6}) (.*)$')
UL_RE = re.compile(r'^- (.*)$')
OL_RE = re.compile(r'^\d+\. (.*)$')


def escape(s):
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')


def inline(s):
    s = escape(s)
    codes = []

    def keep_code(m):
        codes.append(m.group(1))
        return "\x01%d\x02" % (len(codes) - 1)

    s = re.sub(r'`([^`]*)`', keep_code, s)
    s = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', s)
    s = re.sub(r'\*(.+?)\*', r'<em>\1</em>', s)
    s = re.sub(r'\[([^\]]*)\]\(([^)]*)\)', r'<a href="\2">\1</a>', s)
    s = re.sub(r'\x01(\d+)\x02', lambda m: "<code>%s</code>" % codes[int(m.group(1))], s)
    return s


def slugify(raw):
    return re.sub(r'[^a-z0-9]+', '-', raw.lower()).strip('-')


def is_break(line):
    return (
        line.strip() == ''
        or line == '```'
        or HEADING_RE.match(line) is not None
        or line.startswith('- ')
        or OL_RE.match(line) is not None
    )


def main():
    lines = sys.stdin.read().splitlines()
    n = len(lines)
    blocks = []
    headings = []  # (id, plain text)
    used = set()

    i = 0
    while i < n:
        line = lines[i]
        if line.strip() == '':
            i += 1
            continue

        if line == '```':
            j = i + 1
            code = []
            while j < n and lines[j] != '```':
                code.append(escape(lines[j]))
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
                hid = "%s-%d" % (base, k)
                k += 1
            used.add(hid)
            plain = escape(raw.replace('`', '').replace('*', ''))
            headings.append((hid, plain))
            blocks.append('<h%d id="%s">%s</h%d>' % (level, hid, inline(raw), level))
            i += 1
            continue

        if line.startswith('- ') or OL_RE.match(line):
            ordered = not line.startswith('- ')
            items = []
            while i < n:
                cur = lines[i]
                if ordered:
                    om = OL_RE.match(cur)
                    if not om:
                        break
                    items.append(om.group(1).strip())
                else:
                    if not cur.startswith('- '):
                        break
                    items.append(cur[2:].strip())
                i += 1
            tag = 'ol' if ordered else 'ul'
            blocks.append("<%s>\n%s\n</%s>" % (
                tag,
                "\n".join("<li>%s</li>" % inline(t) for t in items),
                tag,
            ))
            continue

        para = []
        while i < n and not is_break(lines[i]):
            para.append(lines[i].strip())
            i += 1
        blocks.append("<p>%s</p>" % inline(" ".join(para)))

    out = []
    if len(headings) >= 2:
        out.append("<nav>")
        for hid, plain in headings:
            out.append('<a href="#%s">%s</a>' % (hid, plain))
        out.append("</nav>")
    out.extend(blocks)
    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
