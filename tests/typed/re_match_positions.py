# A match keeps its groups and their positions: m.start(), m.end(), m.span(), m.groups(), m[n], and the
# places a match can go (a name, a function, a list, a dict, a field, a callback).
import re

m = re.match(r"(a)(b)?(c)?", "ac")
print(m.start(), m.end(), m.span(), m.start(1), m.end(1), m.span(2), m.start(3), m.span(3))
print(m[0], m[1], m.group(0), m.group(3), m.group(2) is None, list(m.groups()), len(m.groups()))
print(m is None, m is not None, bool(m), "yes" if m else "no")
n = re.match(r"z", "abc")
print(n is None, n is not None, bool(n), "yes" if n else "no")
print(re.fullmatch(r"\d+", "12").span(), re.fullmatch(r"\d+", "12a") is None)
p = re.compile(r"(\w)=(\w)")
q = p.search("  a=b")
print(q.start(2), q.end(), list(q.groups()), p.match("  a=b") is None, p.fullmatch("a=b").span(1))
print(re.search("é(.)", "aéb").span(1))


def first(mm):
    return mm.group(1) + ":" + str(mm.start(1))


def find(s):
    return re.search(r"\d(\d)", s)


def find_or_none(s):
    r = re.search(r"\d", s)
    if r is None:
        return None
    return r


print(first(m), first(find("ab12")), find("ab12").span(), find("x") is None)
print(find_or_none("a1").end(), find_or_none("a") is None)

best = None
for s in ["a", "b1", "c22"]:
    t = re.search(r"\d+", s)
    if t:
        best = t
print(best.group(), best.span(), best is None)

ms = [re.search(r"\d", s) for s in ["a1", "22"]]
print(len(ms), [x.start() for x in ms], ms[0].group())
by = {"k": re.search(r"b", "abc")}
print(by["k"].span())


class Tok:
    def __init__(self, m):
        self.m = m
        self.at = m.start()


t = Tok(re.search(r"c", "abc"))
print(t.at, t.m.end(), t.m.group())

its = list(re.finditer(r"(\d)(x)?", "1 2x"))
print(len(its), [i.span() for i in its], [i.span(2) for i in its], [list(i.groups()) for i in its])
print(its)
print([re.match(r"a", "ab")], re.findall(r"\d", "1 2"), re.findall(r"(\d)", "1 2"))


def mark(g):
    return "<" + g.group(1) + "@" + str(g.start()) + "-" + str(g.end(1)) + ">"


print(re.sub(r"(\d)", mark, "a1b2"), re.sub(r"\d", lambda g: str(g.span()), "a1"))
print(re.sub(r"(\d)(x)?", r"[\1\2]", "1 2x"))


class Odd(Exception):
    pass


def boom(g):
    if g.group() == "1":
        raise Odd()
    return "ok"


try:
    print(re.sub(r"\d", boom, "2 1"))
except Odd:
    print("odd")
print(re.sub(r"\d", boom, "2 2"))
