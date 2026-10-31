import sys

lines = sys.stdin.read().splitlines()
W = int(lines[0].strip())
body = lines[1:]

paragraphs = []
cur = []
for line in body:
    if line.strip():
        cur.extend(line.split())
    else:
        if cur:
            paragraphs.append(cur)
            cur = []
if cur:
    paragraphs.append(cur)

outputs = []
for words in paragraphs:
    out_lines = []
    line = ""
    for w in words:
        if not line:
            line = w
        elif len(line) + 1 + len(w) <= W:
            line = line + " " + w
        else:
            out_lines.append(line)
            line = w
    if line:
        out_lines.append(line)
    outputs.append("\n".join(out_lines))

if outputs:
    print("\n\n".join(outputs))
