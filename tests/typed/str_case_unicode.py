# The typed build's str case methods and predicates answer as CPython 3.13 (Unicode 15.1) does for every code
# point, whatever Unicode version rustc ships. Each line is a checksum over all 1,112,064 code points.


def mix(h: int, s: str) -> int:
    i = 0
    for ch in s:
        h = (h * 31 + ord(ch) * (i % 7 + 1)) % 2147483647
        i += 1
    return h


def cps(s: str) -> str:
    return "-".join([str(ord(ch)) for ch in s])


def bit(f: bool) -> int:
    if f:
        return 1
    return 0


hs = [0, 0, 0, 0, 0, 0, 0, 0]
u = 0
while u < 0x110000:
    end = min(u + 4096, 0x110000)
    cs = [chr(x) for x in range(u, end) if x < 0xD800 or x > 0xDFFF]
    chunk = "".join(cs)
    hs[0] = mix(hs[0], chunk.upper())
    hs[1] = mix(hs[1], chunk.lower())
    hs[2] = mix(hs[2], chunk.title())
    hs[3] = mix(hs[3], "".join(["A" + c + "Σ " for c in cs]).lower())
    hs[4] = mix(hs[4], "".join(["a" + c + "a " for c in cs]).title())
    for c in cs:
        hs[5] = mix(hs[5], c.capitalize() + (c + "AΣ").capitalize())
        n = 0
        for f in [c.isupper(), c.islower(), c.isalpha(), c.isalnum(), c.isspace(), ("A" + c).isupper(),
                  ("a" + c).islower(), ("a" + c).isupper(), ("A" + c).islower()]:
            n = n * 2 + bit(f)
        hs[6] = (hs[6] * 4099 + n) % 2147483647
        hs[7] = mix(hs[7], c.casefold())
    u = end
print("upper", hs[0])
print("lower", hs[1])
print("title", hs[2])
print("sigma after cased", hs[3])
print("title after cased", hs[4])
print("capitalize", hs[5])
print("predicates", hs[6])
print("casefold", hs[7])
for s in ["ɤ", "Ɤ", "\U00010d50", "ǆa", "ǅ", "ß", "ﬁx", "ᾳ", "İ", "ა",
          "中a", "ΑΣ", "Α'Σ", "they're", "\x1c"]:
    print(cps(s), cps(s.upper()), cps(s.lower()), cps(s.title()), cps(s.capitalize()),
          s.isupper(), s.islower(), s.isalpha(), s.isalnum(), s.isspace())
