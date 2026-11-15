# re.IGNORECASE follows CPython 3.13's case tables (Unicode 15.1), not the Unicode version of the rustc that
# builds the program. These code points gained case mappings in Unicode 16, 17 or 18, so CPython 3.13 leaves
# them uncased; the classic ones around them must still fold as before.
import re

NEW = [0x19b, 0x264, 0x277, 0x27c, 0x1c89, 0x1c8a, *range(0xa7cb, 0xa7d0), *range(0xa7d2, 0xa7d6),
       *range(0xa7da, 0xa7de), 0xa7e2, 0xab4b, 0xab4c, 0xab6c, 0xab6d, 0x10d50, 0x10d65, 0x10d70, 0x10d85,
       0x16ea0, 0x16eb8, 0x16ebb, 0x16ed3, 0x1df40, 0x1df41, 0x1df4d, 0x1df4e, 0x1df7e, 0x1df7f, 0x1df95]
OLD = [0x41, 0x61, 0x53, 0x73, 0x4b, 0x6b, 0xdf, 0x130, 0x131, 0x17f, 0x1c4, 0x1c5, 0x1c6, 0x3a3, 0x3c2,
       0x3c3, 0x1e9e, 0x212a, 0x2c65, 0x23a, 0xa7c0, 0xa7c1, 0xa7c9, 0xa7ca, 0xa7d0, 0xa7d1, 0xa7d6, 0xa7d7,
       0xa7d8, 0xa7d9, 0x10400, 0x10428, 0x1e900, 0x1e922]
ALL = NEW + OLD
for c in ALL:
    lit = re.compile(re.escape(chr(c)), re.IGNORECASE)
    cls = re.compile("[" + re.escape(chr(c)) + "]", re.I)
    rng = re.compile("(?i)[" + re.escape(chr(c)) + "-" + re.escape(chr(c)) + "]")
    neg = re.compile("[^" + re.escape(chr(c)) + "]", re.I)
    hits = [d for d in ALL if lit.fullmatch(chr(d))]
    same = all(bool(cls.fullmatch(chr(d))) == (d in hits) and bool(rng.fullmatch(chr(d))) == (d in hits)
               and bool(neg.fullmatch(chr(d))) == (d not in hits) for d in ALL)
    print(f"{c:x}", " ".join(f"{d:x}" for d in hits if d != c) or "-", "ok" if same else "class differs")
# backreferences fold the same way
print(bool(re.fullmatch(r"(?i)(.)\1", "ɷɷ")), bool(re.fullmatch(r"(?i)(.)\1", "ɤꟋ")))
print(re.findall("(?i)ɤ", "ɤꟋɤ"), re.sub("(?i)k", "x", "KkK"))
