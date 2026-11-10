import re
# \d \w \s are Unicode classes (unless re.ASCII), and repr escapes what str.isprintable() rejects

for c in "5٣߀Ⅵ½²१éß_   \x1c\x0b-ा":
    print(repr(c), bool(re.match(r"\d", c)), bool(re.match(r"\w", c)), bool(re.match(r"\s", c)), bool(re.match(r"\D", c)), bool(re.match(r"\W", c)), bool(re.match(r"\S", c)),
          bool(re.match(r"[\d]", c)), bool(re.match(r"[\D]", c)), bool(re.match(r"[^\w]", c)), bool(re.match(r"[\W\d]", c)), bool(re.match(r"[a\S]", c)))
    print(bool(re.match(r"\d", c, re.A)), bool(re.match(r"\w", c, re.ASCII)), bool(re.match(r"[\D]", c, re.A)), bool(re.match(r"\s", c, re.A)), bool(re.match(r"\W", c, re.A)))
print(re.findall(r"\w+", "naïve café ٣٤ x_1"), re.sub(r"\d", "#", "a1٣Ⅵ"), re.findall(r"\w+", "naïve café", re.A))
for c in "a\xa0\xad ​͸\U0001f600\U000e0001̀é\x85 ﻿\U0010ffff ~\x7f":
    print(repr(c), repr("x" + c + "y"), [c])
print(repr("  é 　"), {"k ": "\xa0"})
