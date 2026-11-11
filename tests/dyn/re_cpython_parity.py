import re

for p in [r'(?>a*){2}', r'(?>a{,2}?){2}x', r'a**', r'a*(?#c)*', r'a*+*', r'(a*)*b', r'a{2}{3}', r'(a(b)\2)', r'(a(b)\1)']:
    try:
        print(p, re.compile(p).search('aaab') is not None)
    except re.error as e:
        print(p, 'E', e)
for p, f in [('(?a)(?u)', 0), ('(?u)', re.A), ('(?a)', re.U), ('x', re.A | re.U), ('(?i)(?L)', 0)]:
    try:
        re.compile(p, f); print(p, 'ok')
    except Exception as e:
        print(p, type(e).__name__, e)
m = re.match(r'(x)((b)?)', 'x'); print(m.lastindex, m.lastgroup)
m = re.match(r'(a)(b)?', 'a'); print(m.lastindex)
m = re.match(r'((a)(b))', 'ab'); print(m.lastindex)
m = re.match(r'(?P<o>(?P<i>a))', 'a'); print(m.lastindex, m.lastgroup)
m = re.match(r'(a)|(b)', 'b'); print(m.lastindex)
m = re.match(r'(a)*', 'aa'); print(m.lastindex)
m = re.match(r'(a)(?=(b))', 'ab'); print(m.lastindex)
print(re.findall(r'\B', ''), re.findall(r'\B', 'ab'), re.findall(r'(?a:\b)é', 'xé é'), re.search(r'\B', '', 0) )
print(re.findall('(?s:.)', 'a\nb'), re.findall('(?s).(?-s:.)', 'a\nb\n'), re.findall('(?m:^)x', 'x\nx'), re.findall('(?m:x$)', 'x\nx'), re.findall('(?-m:x$)', 'x\nx'))
print(re.findall('(?s:.)', 'a\nb', re.M), re.findall('(?-s:.)', 'a\nb', re.S), re.findall('(?-m:^x)', 'x\nx', re.M))
for p in [r'(?#a\)b)c', r'(?P<a\>b>x)', r'(?P<a\>', r'(x)(?P=a\)', r'(?#a\\)b', r'(?(a\)b)x)', '(?#a\\']:
    try: print(p, re.compile(p).pattern == p, re.findall(p, 'bc x'))
    except re.error as e: print(p, 'E', e)
for t in [r'\g<a\>b>', r'\g<1\>', r'\g<1>\\']:
    try: print(t, re.sub('(?P<a>b)', t, 'abc'))
    except (re.error, IndexError) as e: print(t, type(e).__name__, e)
for p in [r'[é-Zc]', r'[\101-\t]', r'[_-\-]', r'[z-a]', r'[\d-z]', r'[a-\w]', r'[\x41-\x20]', r'x[\U0001F600-a]']:
    try: re.compile(p); print(p, 'ok')
    except re.error as e: print(p, e)
bad = []
for i in range(256):
    c = chr(i)
    for p in ('[' + re.escape(c) + ']', '(?x)' + re.escape(c)):
        try:
            m = re.match(p, c)
            ok = m is not None and m.group(0) == c
        except Exception as e:
            ok = type(e).__name__
        if ok is not True:
            bad.append((i, p, ok))
print(len(bad), bad[:30])
e = re.error('boom', 'pat', 1); print(str(e), e.msg, e.pos, e.pattern, e.lineno, e.colno)
e = re.error('boom'); print(str(e), e.msg, e.pos, e.pattern, e.lineno)
for p, s in [("k", "kKK"), ("ß", "ßẞS"), ("i", "Iiİı"), ("s", "Ssſ"), ("σ", "Σσς"), ("[a-z]+", "KKİıſ-"), ("(.)\\1", "kK")]:
    print(ascii(p), ascii(re.findall(p, s, re.I)), ascii(re.findall(p, s, re.I | re.A)))
for p in [r"a(?<=a*)", r"(a)(?<=\1b*)", r"(?<=ab|c)x", r"(?<=(a)\1)x"]:
    try: print(p, ascii(re.findall(p, "aacx aax")))
    except re.error as e: print(p, "E", e, e.pos)
print(re.findall(r"(?<=\d{3})x", "12x 123x"), re.compile("(?P<a²>x)") if False else "", "a²".isidentifier(), "x·y".isidentifier())
class S(str): pass
print(re.search(S("b+"), S("abbc")).span(), re.sub(S("b"), "x", S("abc")), re.findall("b", S("bab")))
try:
    re.compile("(?<=a{2147483648}a{2147483648})b")
except re.error as e:
    print("error", e)
