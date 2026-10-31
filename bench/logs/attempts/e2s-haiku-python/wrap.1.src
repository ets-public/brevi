import sys


def wrap(words, width):
    lines = []
    cur = ""
    for w in words:
        if cur == "":
            cur = w
        elif len(cur) + 1 + len(w) <= width:
            cur += " " + w
        else:
            lines.append(cur)
            cur = w
    if cur != "":
        lines.append(cur)
    return lines


def main():
    raw = sys.stdin.read().splitlines()
    if not raw:
        return
    width = int(raw[0].strip())
    paragraphs = []
    current = []
    for line in raw[1:]:
        if line.strip() == "":
            if current:
                paragraphs.append(current)
                current = []
        else:
            current.extend(line.split())
    if current:
        paragraphs.append(current)
    blocks = ["\n".join(wrap(p, width)) for p in paragraphs if p]
    if blocks:
        sys.stdout.write("\n\n".join(blocks) + "\n")


main()
