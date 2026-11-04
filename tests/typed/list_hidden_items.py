# A plain list of strings shows every item, whatever the text starts with; regex matches hold their group
# positions apart from it.
import re

xs = ["\x01a", "b"]
print(xs)
print(str(xs), repr(xs), f"{xs}", "%s" % xs)
print(len(xs), xs[0] == "\x01a", xs[-1], xs[:1], xs[1:], xs[::-1])
print(",".join(xs) == "\x01a,b", [len(x) for x in xs], "\x01a" in xs)
for x in xs:
    print(repr(x))
print(xs == ["\x01a", "b"], xs != ["b"], sorted(xs), xs + ["\x01"], [xs, ["\x01"]])
print({"k": xs}, ("\x01z", ["\x01y"]), "a\x01b".split("\x01"), ["\x01"] * 2)
ys = (chr(1) + "1:2,3:4").split(",")
print(ys, len(ys))


def show(items):
    return str(items) + "/" + str(len(items))


print(show(["\x01", "\x01b", "c"]))

# Text from a match that starts with the same character is plain text too.
m = re.search(r"(\x01\w)(\w)?", "x\x01ab")
print(m.group(1) == "\x01a", list(m.groups()), m.span(1), m.span(2), m.start(), m.end(1))
m = re.match(r"(\w)(\x01)", "a\x01")
print(list(m.groups()), m.span(2), m.group(2) == "\x01", len(m.groups()))
print(re.findall(r"\x01\w", "\x01a \x01b"), re.findall(r"(\x01\w)", "\x01a"), re.split(r",", "\x01a,b"))
print(re.sub(r"(\x01)(\w)", r"[\2]", "\x01a"), re.sub(r"\x01", lambda g: "<" + str(g.start()) + ">", "a\x01"))
print([g.group() for g in re.finditer(r"\x01\w", "\x01a \x01b")])
