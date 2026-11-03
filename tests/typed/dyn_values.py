# Results of mixed kinds (a text or a tuple) and texts held as dynamic values.
def score(x):
    if x:
        return ("done", x * 3)
    return "incomplete"


for x in [0, 4]:
    r = score(x)
    if isinstance(r, tuple):
        print("%s: %d" % ("a", r[1]))
    else:
        print("a" + ": " + r)
    print(isinstance(r, (tuple, list)), isinstance(r, str))


def isnum(s):
    return len(s) > 0 and all(c in "0123456789" for c in s)


parts = "tom 6 x".split()
print(isnum(parts[1]), isnum(parts[2]), isnum("patience=12"[9:]), isnum(""))
print(any(c == "x" for c in parts[2]), [c for c in parts[1]])
