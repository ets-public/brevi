# str case methods and predicates answer as CPython 3.13 (Unicode 15.1) does for every code point, whatever
# Unicode version rustc ships: upper, lower (final sigma), title, capitalize, swapcase, casefold, the is* methods,
# and split()/strip() whitespace. Each line is a checksum over all 1,112,064 code points (surrogates excluded).


def mix(h, s):
    i = 0
    for ch in s:
        h = (h * 31 + ord(ch) * (i % 7 + 1)) % 2147483647
        i += 1
    return h


def bits(*fs):
    n = 0
    for f in fs:
        n = n * 2 + (1 if f else 0)
    return n


names = ["upper", "lower", "title", "swapcase", "casefold", "sigma after cased", "sigma at start",
         "sigma before", "swapcase sigma", "title after cased", "capitalize", "predicates", "split strip"]
hs = [0] * len(names)
words = 0
u = 0
while u < 0x110000:
    end = min(u + 4096, 0x110000)
    cs = [chr(x) for x in range(u, end) if not 0xD800 <= x <= 0xDFFF]
    chunk = "".join(cs)
    hs[0] = mix(hs[0], chunk.upper())
    hs[1] = mix(hs[1], chunk.lower())
    hs[2] = mix(hs[2], chunk.title())
    hs[3] = mix(hs[3], chunk.swapcase())
    hs[4] = mix(hs[4], chunk.casefold())
    after = "".join(["A" + c + "Σ " for c in cs])
    hs[5] = mix(hs[5], after.lower())
    hs[6] = mix(hs[6], "".join([c + "Σ " for c in cs]).lower())
    hs[7] = mix(hs[7], "".join(["AΣ" + c + " " for c in cs]).lower())
    hs[8] = mix(hs[8], after.swapcase())
    hs[9] = mix(hs[9], "".join(["a" + c + "a " for c in cs]).title())
    words += len("".join(["x" + c for c in cs]).split())
    for c in cs:
        hs[10] = mix(hs[10], c.capitalize() + (c + "AΣ").capitalize())
        hs[11] = (hs[11] * 4099 + bits(c.isupper(), c.islower(), c.istitle(), c.isalpha(), c.isalnum(), c.isspace(),
                                       ("A" + c).isupper(), ("a" + c).islower(), ("A" + c).istitle(),
                                       (c + "a").istitle(), ("a" + c).isupper(), ("A" + c).islower())) % 2147483647
        hs[12] = (hs[12] * 7 + len((c + "x" + c).strip()) + len((c + "x").lstrip()) + len(("x" + c).rstrip())) % 2147483647
    u = end
for name, h in zip(names, hs):
    print(name, h)
print("split words", words)

# A few by hand: characters rustc and CPython 3.13 disagree on, and CPython's own rules.
for s in ["ɤ", "Ɤ", "\U00010d50", "\U00010d70", "ǆ", "ǆa", "ǅ", "ß", "ﬁx",
          "ᾳ", "İ", "ა", "中a", "ΑΣ", "ΑΣ'", "Α'Σ", "Σ",
          "ΑΣΑ", "they're", "\x1c", "a\x1cb\x1fc"]:
    print(ascii(s), ascii(s.upper()), ascii(s.lower()), ascii(s.title()), ascii(s.capitalize()), ascii(s.swapcase()),
          s.isupper(), s.islower(), s.istitle(), s.isalpha(), s.isalnum(), s.isspace(), ascii(s.split()), ascii(s.strip()))
