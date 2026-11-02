import re
TOK = re.compile(r"\s*(?:('(?:[^']|'')*')|(-?[0-9]+)|([A-Za-z][A-Za-z0-9_]*)|(<=|>=|!=|[(),=<>*]))")
line = "INSERT INTO t VALUES (1, 'it''s')"
pos = 0
while pos < len(line):
    m = TOK.match(line, pos)
    if not m: break
    print(m.groups(), m.lastindex, m.span())
    pos = m.end()
print(re.findall(r"\d+", "a12b345"), re.sub(r"(\w+)@(\w+)", r"\2 at \1", "joe@x and al@y"))
print(re.split(r"[,;]\s*", "a, b;c"), re.split(r"(-)", "1-2-3"), re.sub("x*", "-", "abxd"))
m = re.search(r"(?P<y>\d{4})-(?P<m>\d\d)", "on 2024-05-06")
print(m.groupdict(), m.group("y", "m"), m[0], m.start("m"), repr(m))
print(re.findall(r"(\w)(\d)?", "a1 b c3"), re.escape("a.b*c"), bool(re.fullmatch(r"[a-z]+", "abc")))
print(re.sub(r"(?i)hello", "bye", "Hello HELLO"), re.findall(r"^\w+", "ab cd\nef gh", re.M))
print(re.sub(r"\b(\w)", lambda m: m.group(1).upper(), "the quick fox"))
