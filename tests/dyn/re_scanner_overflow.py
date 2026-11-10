# Pattern.scanner() and repeat counts past CPython's limit (OverflowError), on the dynamic build.
import re
sc = re.compile(r"a*").scanner("baab")
print([ (m.span() if m else None) for m in [sc.search() for _ in range(6)]])
sc = re.compile(r"\d").scanner("1a2")
print([(m.group() if m else None) for m in [sc.match(), sc.match(), sc.match(), sc.search()]])
sc = re.compile(r"x").scanner("axbx", 1)
print(sc.search().span(), sc.search().span(), sc.search(), sc.pattern.pattern)
for p in ("a{4294967295}", "a{4294967294}", "a{1,4294967295}", "a{99999999999999999999}"):
    try:
        re.compile(p); print("ok", p)
    except OverflowError as e:
        print("OverflowError", e)
