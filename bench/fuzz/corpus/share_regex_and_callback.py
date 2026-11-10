import re
def inline(s):
    codes = []
    def sc(m):
        codes.append(m.group(1))
        return "x"
    s = re.sub("a", "b", s)
    return s
ulre = re.compile(r'- (.*)')
lines = ["- a", "- b"]
i = 0
items = []
while i < 2:
    mm = ulre.match(lines[i])
    if not mm:
        break
    items.append(inline(mm.group(1)))
    i += 1
print(items)
