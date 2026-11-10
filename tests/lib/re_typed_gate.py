# Regex syntax the typed build's matcher lacks: the program goes to the dynamic build, which has it, rather
# than matching the pattern wrongly.
import re

m = re.match(r"(?P<year>\d{4})-(?P<month>\d\d)", "2026-11 and more")
print(m.group(1), m.group(2), m.group("year"))
print(re.findall(r"(\w)\1", "aabbcd"))
print(re.findall(r"a(?=b)", "abacab"))
print(re.sub(r"x\Bx", "-", "xx x"))
print(re.findall(r"(?>a+)b", "aab ab"))
print(re.findall(r"a?+", "aab"))
print(re.split(r"(?<=,)", "a,b,c"))
# A compiled pattern used as an object, not only through its match methods.
p = re.compile("[.。]")
print(p.split("a.b。c"), repr(re.compile("x", re.I)), p.pattern)
