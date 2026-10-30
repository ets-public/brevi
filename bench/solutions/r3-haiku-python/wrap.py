import sys


def main():
    lines = sys.stdin.read().split("\n")
    if not lines:
        return
    W = int(lines[0].strip())
    paragraphs = []
    words = []
    for line in lines[1:]:
        toks = line.split()
        if not toks:
            if words:
                paragraphs.append(words)
                words = []
        else:
            words.extend(toks)
    if words:
        paragraphs.append(words)

    blocks = []
    for para in paragraphs:
        out_lines = []
        cur = ""
        for w in para:
            if not cur:
                cur = w
            elif len(cur) + 1 + len(w) <= W:
                cur += " " + w
            else:
                out_lines.append(cur)
                cur = w
        if cur:
            out_lines.append(cur)
        blocks.append("\n".join(out_lines))
    if blocks:
        sys.stdout.write("\n\n".join(blocks) + "\n")


main()
