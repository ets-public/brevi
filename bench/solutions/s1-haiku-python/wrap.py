import sys

lines = sys.stdin.read().splitlines()
W = int(lines[0].strip())
body = lines[1:]

paras = []
cur = []
for line in body:
    if line.strip() == "":
        if cur:
            paras.append(cur)
            cur = []
    else:
        cur.extend(line.split())
if cur:
    paras.append(cur)

out_paras = []
for words in paras:
    out_lines = []
    line = ""
    for w in words:
        if not line:
            line = w
        elif len(line) + 1 + len(w) <= W:
            line += " " + w
        else:
            out_lines.append(line)
            line = w
    if line:
        out_lines.append(line)
    out_paras.append("\n".join(out_lines))

print("\n\n".join(out_paras))
