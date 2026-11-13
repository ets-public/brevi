# re over code points, lone surrogates included, as CPython's sre runs it (handoffs/string-model-design.md, step 3):
# a surrogate is a code point like any other, never a word, digit or space character, and it folds to itself.
import re

D, E, L = chr(0xD800), chr(0xDC80), chr(0xDFFF)
P, Q = chr(0xF0000), chr(0xF0480)  # plane-15 private-use characters: they must stay distinct from every surrogate
s = 'ab' + D + 'cd' + E + ' x' + L

# stdlib test_re.test_bigcharset
print(re.match("([\u2222\u2223])", "\u2222").group(1) == "\u2222")
r = '[%s]' % ''.join(map(chr, range(256, 2**16, 255)))
print(ascii(re.match(r, "\uff01").group()), re.match(r, D) is None, ascii(re.match(r, chr(0xD828)).group()))

# a subject with surrogates
print(ascii(re.search(D, s).group()), re.search(D, s).span(), ascii(re.findall('.', D + E)), ascii(re.findall(r'\w+', s)))
print(ascii(re.findall(r'\W', s)), ascii(re.findall(r'\S', D + ' ' + E)), re.findall(r'\d', D + '1'), re.findall(r'\s', D))
print(ascii(re.sub(D, '!', s)), ascii(re.sub('c', E, 'acb')), ascii(re.subn('[a-z]', D, 'a1b')), ascii(re.split(E, s)))
print(ascii(re.sub(r'(\w)', r'<\1>', 'a' + D)), ascii(re.sub('.', lambda m: m.group() + D, 'xy')), ascii(re.escape('a.b')))
print(ascii(re.fullmatch('ab.cd.', s[:6]).group()), ascii([m.group() for m in re.finditer('[' + D + E + ']', s)]))
print(re.match(r'\b', D) is None, re.search(r'\B', D) is not None, ascii(re.findall(r'\b\w', D + 'a' + E + 'b')))

# ranges, classes and escapes over the surrogate block
print(ascii(re.findall('[\ud800-\udfff]', s)), ascii(re.findall(r'[\ud800-\udfff]', s)), ascii(re.findall('[^a-z ]', s)))
print(ascii(re.findall(r'\ud800|\udfff', s)), re.match(r'\udc80', E) is not None, ascii(re.findall('[' + D + '-' + L + ']+', s)))
print(re.match(D, P) is None, re.match(P, D) is None, re.match('[' + P + ']', D) is None, re.match('[' + D + ']', P) is None)
print(re.match(r'[\U000f0000-\U000f07ff]', D) is None, ascii(re.match('[\ud7ff-\ue000]+', D + E + L).group()))
print(ascii(re.match('.', D, re.DOTALL).group()), re.match(D, E) is None, re.match(D + '+', D * 3).end())

# IGNORECASE: surrogates are uncased; ranges over them fold
print(ascii(re.findall(D, D + D.upper(), re.I)), ascii(re.findall('[' + D + '-' + E + ']', 'A' + D + E, re.I)), re.match('[\ud7ff-\ue000a-z]', D, re.I) is not None)
print(ascii(re.findall('(' + D + r')\1', D * 2 + E * 2, re.I)), ascii(re.findall(r'(.)\1', D + D + E + L, re.I)))

# patterns holding lone surrogates: the Pattern object, its text and its errors
p = re.compile(D + '(' + E + ')' + 'x')
print(ascii(p.pattern), ascii(repr(p)), p.groups, p == re.compile(D + '(' + E + ')' + 'x'), p == re.compile(P + '(' + Q + ')x'))
q = re.compile(P + '(' + Q + ')x')  # the placeholder text of p's pattern: a different pattern
print(q.match(D + E + 'x') is None, ascii(q.match(P + Q + 'x').group(1)), q == p, ascii(re.compile(D + '(' + E + ')' + 'x').pattern))
print(ascii(p.search('a' + D + E + 'x').group(1)), ascii(p.sub('-', D + E + 'x' + D)), p.flags)
print(re.compile(P + 'x').match(D + 'x') is None, re.compile(D + 'x').match(P + 'x') is None)
for bad in ['(' + D, D + '[', D + ')', '[' + D, D + '*?+' + '{', '(?P<a>' + D + ')(?P=b)']:
    try:
        re.compile(bad)
        print('compiled', ascii(bad))
    except re.error as e:
        print(ascii(e.msg), e.pos, ascii(e.pattern), ascii(str(e)))

# groups, groupdict, and the match's text
m = re.match('(?P<a>.)(?P<b>[^x]*)', D + 'b' + E)
print(ascii(m.groups()), ascii(m.groupdict()), ascii(m.group(0)), ascii(m.string), m.start('b'), m.end(), ascii(m[1]))
print(ascii(m.expand(r'\g<b>-\1')), ascii(re.match('(a)|(' + D + ')', D).groups()), len(re.match('.*', s).group()))

# replacement templates holding lone surrogates
print(ascii(re.sub('(b)', D + r'\1' + E, 'abc')), ascii(m.expand(D + r'\g<a>')), ascii(re.sub('x', D, 'xyx', count=1)))
for bad in [D + '\\q', D + '\\g<9>', '\\g<' + D]:
    try:
        re.sub('(a)', bad, 'a')
    except re.error as e:
        print(ascii(e.msg), e.pos, ascii(e.pattern))

# json: a lone \u escape loads as a lone surrogate; ensure_ascii writes one as an escape
import json
B = chr(92)
j = json.loads('["' + B + 'ud800", "a' + B + 'udc80b", "' + B + 'ud83d' + B + 'ude00", "' + B + 'udfff' + B + 'ud800", {"k": "' + B + 'udbff"}]')
print(ascii(j), len(j[2]), ascii(json.dumps(D + 'x' + E)), ascii(json.dumps([L, {'k': D}])), json.loads(json.dumps(D)) == D)
print(ascii(repr(re.match(D + 'a', D + 'ab'))), ascii(repr(re.compile(E * 3, re.I))))
