import re
P = re.compile(r"^ab")
Q = re.compile(r"\bcd")
R = re.compile(r"(x)?(y)")
s = "abcd ab cd xy y"
print(bool(P.match(s, 0)), bool(P.match(s, 5)))
print(bool(Q.match(s, 2)), bool(Q.match(s, 8)))
m = R.search(s, 3)
print(m.group(0), m.group(1), m.group(2), m.start(), m.end(), m.span())
m = R.search(s, 13)
print(m.group(1) is None, m.group(2), m.end())
print(R.match(s, 100) is None, R.search(s, -5).span())
print(re.compile(r"\d+").search("ab12cd345", 4).group())
