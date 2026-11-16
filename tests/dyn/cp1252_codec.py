# The cp1252 codec (CPython's charmap table) in bytes.decode, str.encode, bytes(), str(), bytearray (#589).
HANDLERS = ["strict", "replace", "ignore", "backslashreplace", "xmlcharrefreplace", "surrogateescape", "surrogatepass"]
DEC = ["strict", "replace", "ignore", "backslashreplace", "surrogateescape", "surrogatepass"]


def dec(b, enc, h):
    try:
        return ascii(b.decode(enc, h))
    except UnicodeDecodeError as e:
        return "UDE %s | %s %d %d %s %s" % (e, e.encoding, e.start, e.end, e.reason, ascii(e.object)[:60])


def enc(s, e, h):
    try:
        return ascii(s.encode(e, h))
    except UnicodeEncodeError as x:
        return "UEE %s | %s %d %d %s %s" % (x, x.encoding, x.start, x.end, x.reason, ascii(x.object)[:60])


def digest(parts):
    h = 0
    for p in parts:
        for ch in p:
            h = (h * 131 + ord(ch)) % 1000000007
    return "%d %d" % (len(parts), h)


for h in DEC:
    print(h, dec(bytes(range(256)), "cp1252", h))
    print(h, digest([dec(bytes([i]), "cp1252", h) for i in range(256)]))
    for i in (0x41, 0x80, 0x81, 0x8D, 0x8F, 0x90, 0x9D, 0xFF):
        print(h, i, dec(bytes([i]), "cp1252", h))
    print(h, dec(b"a\x81b\x8d\x8f\x90\x9dz", "cp1252", h))

for h in HANDLERS:
    allc = "".join(chr(i) for i in range(0x2200)) + "\U00010080\U0001f600"
    print(h, digest([enc(allc, "cp1252", h)[:80], enc(allc, "cp1252", h)[-80:]]), len(enc(allc, "cp1252", h)))
    print(h, digest([enc(chr(i), "cp1252", h) for i in range(0x2200)]))
    for i in (0x41, 0x7F, 0x80, 0x152, 0x20AC, 0x2022, 0x4E00, 0xD800, 0xDC80, 0xDCFF):
        print(h, i, enc(chr(i), "cp1252", h))
    for s in ["a\u4e00b", "\u4e00\u4e01x\u4e02", "\U00010080", "a\udc80b", "a\udc80\udc81", "\udc41\udc80", "a\udc80\udc41\u4e00", "\ud800", "\u20ac\u0152\u0161", "\u20acx\u2603\U0001f600y"]:
        print(h, ascii(s), enc(s, "cp1252", h))

for n in ["cp1252", "CP1252", "windows-1252", "Windows_1252", "1252", "windows 1252", "windows.1252", "Cp1252 ", "WINDOWS--1252", "-1252"]:
    try:
        print(n, ascii("€".encode(n)), ascii(b"\x80".decode(n)))
    except LookupError as x:
        print(n, "LookupError", x)

print(ascii(bytes("€\xe9", "cp1252")), ascii(str(b"\x80\xe9", "cp1252")), ascii(bytearray(b"\x93hi\x94").decode("cp1252")))
print(ascii(bytes("一", "cp1252", "replace")), ascii(str(b"\x81", "cp1252", "replace")))
print(ascii(bytearray("€", "cp1252")), ascii("€".encode(encoding="cp1252", errors="strict")))
try:
    bytes("一", "cp1252")
except UnicodeEncodeError as x:
    print(x, x.args)
try:
    str(b"\x81", "cp1252")
except UnicodeDecodeError as x:
    print(x, x.args)
print(b"".decode("cp1252"), "".encode("cp1252"))
