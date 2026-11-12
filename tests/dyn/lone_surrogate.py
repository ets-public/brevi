# A str can hold a lone surrogate (U+D800..U+DFFF), as in CPython: literals, chr(), and the operations the
# dynamic build handles for such strs so far (handoffs/string-model-design.md, step 1).
a = '123\ud800'
b = "\udfff"
c = chr(0xD800)
d = '\U0000dc80x' 'y'
e = 'x' '\ud83d' '\ude00'
print(repr(a), repr(b), repr(c), repr(d), repr(e), ascii(a))
print(len(a), len(b), len(c), len(d), len(e))
print(a == '123\ud800', a == '123', c == '\ud800', a != c, e == '\U0001f600')
print(a + 'z' == '123\ud800z', repr(c + c), repr('q' + c), len(c + c))
print(c in a, '12' in a, 'x' in c, c in c + chr(0xdc00), '' in c, chr(0xdc00) in a)
print(sorted([c, 'a', chr(0xffff), '\U00010000', b]) == ['a', c, b, chr(0xffff), '\U00010000'])
print(a < '124', a > '123', c < b, chr(0xfb00) > c)
print(ord(c), ord(b), bool(c), hash(a) == hash('123' + chr(0xd800)), len({a, '123\ud800', c}))
print({a: 1}['123\ud800'], str(a) is a, a is a)
print("%r and %r" % (a, b), "%s" % 5)
print(f"{a!r}", "{!r}".format(b))
for s, base in [(a, None), (a, 10), (c, None), ('  7\udc80 ', 16), ('1' * 5000 + '\ud800', None)]:
    try:
        int(s) if base is None else int(s, base)
    except ValueError as ex:
        print(str(ex)[:70])
try:
    int(a, 99)
except ValueError as ex:
    print(ex)
