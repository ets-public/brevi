import re

# bytes patterns and subjects in the dynamic build
print(re.match(b"abc", b"ABC", re.I).group(0))
print(re.search(br"\b(b.)\b", b"abcd abc bcd bx", re.LOCALE).group(1))
print(re.findall(br"\w+", b"ab \xe9\x80"))
print(re.sub(b"(a)", b"\\g<1>-\\1", b"1a2"))
print(re.split(b"(a)?", b"1a2"))
print(re.escape(b"a.b(c)"))
print(repr(re.compile(b"ab", re.I | re.LOCALE)))
print(repr(re.match(b"ab", b"ab")))
print(re.search(br"\B", b"abc").span())
try:
    re.match(b"a", "a")
except TypeError as e:
    print("mix", e)
try:
    re.match("a", b"a")
except TypeError as e:
    print("mix", e)
