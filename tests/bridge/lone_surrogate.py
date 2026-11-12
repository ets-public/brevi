# A str with lone surrogates crossing into a library CPython runs and back (#450 step 4): built there from its
# code points, read back the same way.
from brlib import describe, apply, fill

L, E = chr(0xD800), chr(0xDC80)
print(describe(L), describe("a" + E + "b"), describe([L, "ok"]))
r = apply(lambda s: s + E, [L, "x", "y" + L])
print([ascii(x) for x in r], [len(x) for x in r], r[0] == L + E)
d = {L: E}
xs = [L]
fill(xs, d)
print(ascii(xs), ascii(sorted(d.items(), key=repr)))
print(ascii(apply(lambda s: s, [L], )[0]), apply(len, [L + E + "z"]))
