# The typed build's re.IGNORECASE answers as CPython 3.13's re does (its _sre case tables and
# re._casefix's extra cases, Unicode 15.1), whatever Unicode version rustc ships. Each line is a checksum.
import re


def mix(h: int, s: str) -> int:
    for ch in s:
        h = (h * 31 + ord(ch)) % 2147483647
    return h


def esc(u: int) -> str:
    return f"\\U{u:08x}"


# every code point with a case mapping, and what it maps to: the only text a cased pattern can match
# besides the pattern's own characters
cased = []
u = 0
while u < 0x110000:
    if u < 0xD800 or u > 0xDFFF:
        c = chr(u)
        if c.lower() != c or c.upper() != c:
            cased.append(u)
    u += 1
text = "".join([chr(x) for x in cased]) + "".join([chr(x) for x in range(0x20, 0x7f)])
print("cased", len(cased))

# 1. each cased character as a literal, a one-character set and a negated set, under Unicode and ASCII rules
h = [0, 0, 0, 0]
for x in cased:
    h[0] = mix(h[0], "".join(re.findall("(?i)" + esc(x), text)))
    h[1] = mix(h[1], "".join(re.findall("(?i)[" + esc(x) + "]", text)))
    h[2] = mix(h[2], "".join(re.findall("(?i)[^" + esc(x) + "]", text)))
    h[3] = mix(h[3], "".join(re.findall("(?i)(?a)" + esc(x), text)))
print("literal", h[0])
print("set", h[1])
print("negated set", h[2])
print("ascii literal", h[3])

# 2. every code point but the surrogates (no str the typed build makes holds one), in sets of 256 written one
# by one, as one range and as a run of 256 literals: an uncased character matches only itself
hs = [0, 0, 0, 0]
lo = 0
while lo < 0x110000:
    if lo == 0xD800:
        lo = 0xE000
    hi = lo + 255
    own = "".join([chr(x) for x in range(lo, hi + 1)])
    t = text + own
    hs[0] = mix(hs[0], "".join(re.findall("(?i)[" + "".join([esc(x) for x in range(lo, hi + 1)]) + "]", t)))
    hs[1] = mix(hs[1], "".join(re.findall("(?i)[" + esc(lo) + "-" + esc(hi) + "]", t)))
    hs[2] = mix(hs[2], "".join(re.findall("(?i)(?a)[" + esc(lo) + "-" + esc(hi) + "]", t)))
    hs[3] = mix(hs[3], "".join(re.findall("(?i)" + "".join([esc(x) for x in range(lo, hi + 1)]), t)))
    lo += 256
print("sets of 256", hs[0])
print("ranges of 256", hs[1])
print("ascii ranges of 256", hs[2])
print("runs of 256 literals", hs[3])

n = 0
# 3. ranges past U+FFFF (RANGE_UNI_IGNORE: the folded character or its uppercase), sets with classes in them
for p in ["(?i)[\U00010000-\U0010ffff]", "(?i)[^\U00010000-\U0010ffff]", "(?i)[Ā-\U00010400]", "(?i)[\U00010428-\U0001044f]",
          "(?i)(?a)[\U00010428-\U0001044f]", "(?i)[\U0001e922-\U0001e943]", "(?i)[\x00-\U0010ffff]", "(?i)[k\\d]", "(?i)[^k\\W]",
          "(?i)[\\w]", "(?i)[^\\sſ]", "(?i)(?a)[\\wK]", "(?i)[İ-ı]", "(?i)[A-Za-z]+", "(?i)σ+"]:
    n += 1
    print("pattern", n, mix(0, "".join(re.findall(p, text))))
