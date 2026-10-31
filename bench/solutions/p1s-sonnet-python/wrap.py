import sys
lines = sys.stdin.read().split('\n')
w = int(lines[0])
paras = []
cur = []
for line in lines[1:]:
    if line.strip() == '':
        if cur:
            paras.append(cur)
            cur = []
    else:
        cur.extend(line.split())
if cur:
    paras.append(cur)
out = []
for words in paras:
    ls = []
    line = ''
    for wd in words:
        if not line:
            line = wd
        elif len(line) + 1 + len(wd) <= w:
            line += ' ' + wd
        else:
            ls.append(line)
            line = wd
    if line:
        ls.append(line)
    out.append("\n".join(ls))
print("\n\n".join(out))
