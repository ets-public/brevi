# A str with lone surrogates through encode/decode, print, sys.stdout/stderr, json and re.escape (#450 step 4).
import json, re, sys

L, E, F = chr(0xD800), chr(0xDC80), chr(0xDFFF)
samples = ["a" + L + F + "b", "x" + E + chr(0xDCFF) + "y", chr(0xE9) + E, E + chr(0xE9), "a" + L + chr(0xE9), L + E, E + L, "plain", chr(0xE9) + "t"]
for s in samples:
    for enc in ["utf-8", "ascii", "latin-1", "UTF8", "us-ascii"]:
        for err in ["strict", "surrogatepass", "surrogateescape", "backslashreplace", "xmlcharrefreplace", "replace", "ignore"]:
            try:
                r = repr(s.encode(enc, err))
            except UnicodeEncodeError as e:
                r = "UnicodeEncodeError: %s | %s %s %d %d %s" % (e, e.encoding, ascii(e.object), e.start, e.end, e.reason)
            print(ascii(s), enc, err, r)

print(ascii(bytes(L + "z", "utf-8", "surrogatepass")), ascii(bytearray(E, "latin-1", "surrogateescape")))
print(ascii((L + "q").encode(errors="backslashreplace")), ascii((L + "q").encode(encoding="utf-8", errors="surrogatepass")))
try:
    bytes(L, "utf-8")
except UnicodeEncodeError as e:
    print("bytes()", e, e.args[2:])

for b in [b"a\x80b", b"\xed\xa0\x80", b"\xed\xa0\x80\xed\xb2\x80x", b"\xed\xa0", b"\xe9A", b"\xf0\x9f\x98", b"\xff\xfe", b"\xed\xa0A", b"\xc3", b"ok", b"\xed\xbf\xbf"]:
    for enc in ["utf-8", "ascii", "latin-1"]:
        for err in ["surrogateescape", "surrogatepass", "strict"]:
            try:
                r = b.decode(enc, err)
                r = ascii(r) + " " + str(len(r))
            except UnicodeDecodeError as e:
                r = "UnicodeDecodeError: %s" % e
            print(b, enc, err, r)
# round trips
raw = bytes(range(0x7c, 0x100))
t = raw.decode("utf-8", "surrogateescape")
print(len(t), t.encode("utf-8", "surrogateescape") == raw, ascii(t[:8]))
t = (L + F + "k").encode("utf-8", "surrogatepass").decode("utf-8", "surrogatepass")
print(ascii(t), t == L + F + "k")

# a hand-built UnicodeEncodeError
e = UnicodeEncodeError("utf-8", "a" + L, 1, 2, "surrogates not allowed")
print(e, ascii(e.args[1]), e.args[2:])
print(UnicodeEncodeError("ascii", "abc", 0, 3, "nope"))

# print and sys.stdout: stdout raises for \ud800 under both error handlers python3 may pick for it (strict, or
# surrogateescape under the C locale; check.sh tests the C locale's raw bytes); stderr writes \udXXX (backslashreplace)
for args in [(L,), ("before", L, "after"), ("ok", 1)]:
    try:
        print(*args)
    except UnicodeEncodeError as e:
        print()
        print("print raised:", e, e.start, e.end, ascii(e.object))
try:
    print("x", end=L)
except UnicodeEncodeError as e:
    print("|end raised:", e)
try:
    print("p", "q", sep=L)
except UnicodeEncodeError as e:
    print("|sep raised:", e)
try:
    sys.stdout.write("w" + L)
except UnicodeEncodeError as e:
    print("write raised:", e)
sys.stdout.flush()
print("to stderr:", L + "z", file=sys.stderr)
sys.stderr.flush()
n = sys.stderr.write("err" + E + "\n")
sys.stderr.flush()
print("stderr wrote", n)

class Sink:
    def __init__(self):
        self.parts = []
    def write(self, t):
        self.parts.append(t)
    def flush(self):
        pass
k = Sink()
print("a", L, sep=E, end=F, file=k)
print([ascii(p) for p in k.parts])

# json
d = {"k" + L: "v" + E, "n": [F, "é", 1]}
print(json.dumps(d))
print(ascii(json.dumps(d, ensure_ascii=False)))
print(ascii(json.dumps(d, ensure_ascii=False, sort_keys=True, indent=1)))
print(ascii(json.dumps(L, ensure_ascii=False)), len(json.dumps(L, ensure_ascii=False)))
print(json.loads(json.dumps(d)) == d)
print(ascii(json.JSONEncoder(ensure_ascii=False).encode([E])))
k = Sink()
json.dump({L: 1}, k, ensure_ascii=False)
print(ascii("".join(k.parts)))

# re.escape
print(ascii(re.escape("a.b" + L + "*" + E)))
print(re.fullmatch(re.escape(L + ".+"), L + ".+") is not None)

# re.Scanner with a lone phrase and subject
sc = re.Scanner([(r"[a-z]+", lambda s, t: ("w", t)), (L + "+", lambda s, t: ("L", len(t))), (r"\s+", None)])
print(ascii(sc.scan("ab " + L + L + " cd" + chr(0xDC00))))
