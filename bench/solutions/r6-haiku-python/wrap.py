import sys


def wrap_para(words, W):
    lines = []
    cur = ""
    for w in words:
        if not cur:
            cur = w
        elif len(cur) + 1 + len(w) <= W:
            cur += " " + w
        else:
            lines.append(cur)
            cur = w
    if cur:
        lines.append(cur)
    return "\n".join(lines)


def main():
    raw = sys.stdin.read().split("\n")
    W = int(raw[0].strip())
    paras = []
    words = []
    for line in raw[1:]:
        if line.strip() == "":
            if words:
                paras.append(words)
                words = []
        else:
            words.extend(line.split())
    if words:
        paras.append(words)
    print("\n\n".join(wrap_para(p, W) for p in paras))


main()
