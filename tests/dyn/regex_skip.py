# Searches that skip ahead to a possible first character, and one-character patterns (#195).
import re

text = "Morning coffee, then the GYM. morning again; Ünïcode ünïcode xmorning"
for w in ["morning", "gym", "ünïcode", "then", "x", "zzz"]:
    m = re.search(rf"\b{w}", text, re.I)
    print(w, m and (m.start(), m.group()), len(re.findall(rf"\b{w}", text, re.I)), re.search(rf"\B{w}", text, re.I) is not None)
print(re.findall(r"(?m)^\w+", "ab cd\nef gh\n\nij"))
print(re.sub(r"[^a-z ]", "", "Hello, World! 42 times"), re.sub(r"[^a-z ]", "", ""))
print(re.split(r"[,;]", "a,b;;c"), re.findall(r"\d", "a1b22c333"), re.sub(r"\s", "_", "a b\tc\n"))
print(re.search(r"[xy]z", "aaxbyz").span(), re.search(r"\Afoo", "foofoo").span(), re.search(r"\Afoo", "barfoo"))
print(re.compile(r"o").search("foo", 2).span(), re.compile(r"o").search("foo", 3), re.compile(r"[ab]").search("xxab", 1, 3))
print(re.fullmatch(r"[a-z]", "q") is not None, re.fullmatch(r"[a-z]", "qq"), re.match(r"\d", "5x").group())
s = "the same text, searched many times"
print([bool(re.search(rf"\b{w}", s)) for w in ["the", "text", "many", "nope", "times", "s"]])
