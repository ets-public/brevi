import re
TOK = re.compile(r"\s*(?:(\d+)|([a-z]+))")
s = "12 ab 7"
pos = 0
while pos < len(s):
    m = TOK.match(s, pos)
    if not m:
        break
    if m.group(1) is not None:
        print("n", int(m.group(1)))
    else:
        print("id", m.group(2))
    pos = m.end()
