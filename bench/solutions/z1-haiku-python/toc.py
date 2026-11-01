import sys
import re

HEADING_RE = re.compile(r"^(#{1,6}) (.*)$")
CLOSING_RE = re.compile(r"\s+#+$")
SLUG_BAD_RE = re.compile(r"[^a-z0-9 \-]")


def make_slug(text):
    s = SLUG_BAD_RE.sub("", text.lower())
    s = s.replace(" ", "-")
    if s == "":
        s = "section"
    return s


def main():
    lines = sys.stdin.read().split("\n")
    if lines and lines[-1] == "":
        lines.pop()

    headings = []  # (lineno, level, text)
    warnings = []  # (lineno, message)
    in_code = False
    open_line = None

    for idx, raw in enumerate(lines):
        lineno = idx + 1
        line = raw.rstrip("\r")
        if line.startswith("```"):
            if in_code:
                in_code = False
                open_line = None
            else:
                in_code = True
                open_line = lineno
            continue
        if in_code:
            continue
        m = HEADING_RE.match(line)
        if not m:
            continue
        text = m.group(2).strip()
        text = CLOSING_RE.sub("", text) if text else text
        if text == "":
            continue
        headings.append((lineno, len(m.group(1)), text))

    if in_code and open_line is not None:
        warnings.append((open_line, "line %d: unclosed code block" % open_line))

    if not headings:
        for _, msg in sorted(warnings):
            print(msg)
        print("no headings")
        return

    top = min(level for _, level, _ in headings)

    depths = []
    prev_level = None
    prev_depth = None
    for lineno, level, text in headings:
        if prev_level is None:
            depth = 1 + (level - top)
        elif level - prev_level > 1:
            warnings.append((lineno, "line %d: level jump" % lineno))
            depth = prev_depth + 1
        else:
            depth = 1 + (level - top)
        depths.append(depth)
        prev_level = level
        prev_depth = depth

    for _, msg in sorted(warnings):
        print(msg)

    counters = [0] * 64
    seen = {}
    out = []
    max_depth = 0
    for (lineno, level, text), depth in zip(headings, depths):
        counters[depth] += 1
        for k in range(depth + 1, len(counters)):
            counters[k] = 0
        number = ".".join(str(counters[k]) for k in range(1, depth + 1))

        plain = make_slug(text)
        n = seen.get(plain, 0)
        slug = plain if n == 0 else "%s-%d" % (plain, n)
        seen[plain] = n + 1

        out.append("  " * (depth - 1) + "%s %s (#%s)" % (number, text, slug))
        max_depth = max(max_depth, depth)

    for line in out:
        print(line)
    print("%d headings, max depth %d" % (len(headings), max_depth))


if __name__ == "__main__":
    main()
