import sys

data = sys.stdin.read().split("\n")
W = int(data[0].strip())

paras = []
cur = []
for line in data[1:]:
    words = line.split()
    if not words:
        if cur:
            paras.append(cur)
            cur = []
    else:
        cur.extend(words)
if cur:
    paras.append(cur)

out_paras = []
for words in paras:
    lines = []
    line = ""
    for w in words:
        if not line:
            line = w
        elif len(line) + 1 + len(w) <= W:
            line += " " + w
        else:
            lines.append(line)
            line = w
    if line:
        lines.append(line)
    out_paras.append("\n".join(lines))

print("\n\n".join(out_paras))
