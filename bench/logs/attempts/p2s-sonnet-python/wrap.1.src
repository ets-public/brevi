import sys
lines = sys.stdin.read().split('\n')
w = int(lines[0].strip())
paras = []
cur = []
for line in lines[1:]:
    if line.strip():
        cur.extend(line.split())
    else:
        if cur:
            paras.append(cur)
        cur = []
if cur:
    paras.append(cur)
outp = []
for words in paras:
    res = []
    line = ""
    for wd in words:
        if not line:
            line = wd
        elif len(line) + 1 + len(wd) <= w:
            line += " " + wd
        else:
            res.append(line)
            line = wd
    if line:
        res.append(line)
    outp.append("\n".join(res))
print("\n\n".join(outp))
