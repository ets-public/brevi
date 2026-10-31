import sys

lines = sys.stdin.read().split("\n")
W = int(lines[0].strip())

paras = []
cur = []
for line in lines[1:]:
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
    out = []
    line = ""
    for w in words:
        if not line:
            line = w
        elif len(line) + 1 + len(w) <= W:
            line += " " + w
        else:
            out.append(line)
            line = w
    if line:
        out.append(line)
    out_paras.append("\n".join(out))

print("\n\n".join(out_paras))
