import sys

lines = sys.stdin.read().splitlines()
W = int(lines[0].strip())

paras = []
cur = []
for raw in lines[1:]:
    if raw.strip() == "":
        if cur:
            paras.append(cur)
            cur = []
    else:
        cur.extend(raw.split())
if cur:
    paras.append(cur)

out = []
for words in paras:
    wrapped = []
    line = None
    for w in words:
        if line is None:
            line = w
        elif len(line) + 1 + len(w) <= W:
            line += " " + w
        else:
            wrapped.append(line)
            line = w
    if line is not None:
        wrapped.append(line)
    out.append("\n".join(wrapped))

print("\n\n".join(out))
