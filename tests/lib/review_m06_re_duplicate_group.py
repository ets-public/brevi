import re
try:
    re.compile(r"(?P<x>a)(?P<x>b)")
    print("compiled")
except re.error as e:
    print("err",e)
m=re.match(r"(?P<y>\d+)-(\d+)","12-07")
print(m.re.groupindex, m.lastindex, m.lastgroup)

# the same duplicate name nearby: a longer name, after an unnamed group, nested
# inside another group, repeated, and in an alternation
for pat in [r"(?P<xx>a)(?P<xx>b)", r"(?P<x>a)(?:c)(?P<x>b)", r"((?P<x>a))(?P<x>b)", r"(?P<x>a)*(?P<x>b)", r"(?P<x>a)|(?P<x>b)"]:
    try:
        re.compile(pat)
        print(repr(pat), "compiled")
    except re.error as e:
        print(repr(pat), "err", e)
