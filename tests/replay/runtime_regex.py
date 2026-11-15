# Patterns built at run time with syntax the typed matcher lacks (back-references, named groups,
# look-arounds, lone surrogate escapes, a bad repeat): the build cannot see them, so the typed run stops
# when it compiles one and `brevi run` re-runs the program on the dynamic build. Never a wrong match.
import re
import sys

word = sys.argv[0][:0] + "a"
print("before", re.search(word + "+", "caaab").span())
back = "(" + word + ")" + "\\1"
m = re.search(back, "xaay")
print("search", m.group(0) if m else None, m.span() if m else None)
print("match", re.match(back, "aab") is not None)
print("fullmatch", re.fullmatch(back + "b", "aab") is not None)
print("sub", re.sub(back, "<>", "aaaa"))
print("split", re.split("(?=" + word + ")", "bab"))
print("finditer", [x.group(0) for x in re.finditer("(?P<w>" + word + ")", "aba")])
c = re.compile("\\ud800" + word)
print("compile", c.search("a") is None)
try:
    re.compile(word + "**")
except re.error as e:
    print("error", e)
print("after")
