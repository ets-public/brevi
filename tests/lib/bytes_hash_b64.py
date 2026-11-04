# bytes values, hashlib and base64 build natively (dynamic build), printing what python3 prints.
import base64
import binascii
import hashlib
from hashlib import sha256

b = b"hello\x00\xff'"
print(b, len(b), b[0], b[-1], b[1:4], b[::-2], list(b[:3]), b"ell" in b, 104 in b, bool(b""))
print(b"a" b"b", rb"\n", b"\\u12", b"\x41\101", repr(b'"'), repr(b"'\""))
print("héllo".encode(), "héllo".encode("utf-8").decode(), b"abc".decode("ascii"), "é".encode("latin-1"))
print(bytes(3), bytes([65, 66]), bytes("ab", "utf-8"), bytes(b"x"), bytes.fromhex("de ad be ef").hex(), b"\x01\xab".hex(":"))
print(b"a,b,,c".split(b","), b"  x y ".split(), b" x ".strip(), b"-".join([b"a", b"b"]), b"ab" + b"cd", b"ab" * 2, 2 * b"z")
print(isinstance(b, bytes), type(b).__name__, b == b"hello\x00\xff'", b == "hello", {b"k": 1}[b"k"], sorted([b"b", b"a", b"ab"]))
print(b"x".upper(), b"abc".startswith((b"x", b"ab")), b"abc".find(b"c"), b"abca".count(b"a"), b"aXbX".replace(b"X", b"-"), b"12".isdigit())
print(f"{b'xy'}", str(b"q"), [c for c in b"AB"], b"line1\nline2\r\n".splitlines())
for name in ["md5", "sha1", "sha224", "sha256", "sha384", "sha512"]:
    print(name, hashlib.new(name, b"The quick brown fox").hexdigest())
h = sha256()
h.update(b"a")
h2 = h.copy()
h.update(b"bc")
print(h.hexdigest(), h2.hexdigest(), h.name, h.digest_size, h.block_size, len(h.digest()))
print(hashlib.md5("x".encode()).hexdigest(), hashlib.sha1(b"").hexdigest(), hashlib.sha512(b"a" * 200).hexdigest()[:20])
print(base64.b64encode(b"hello world"), base64.b64decode("aGVsbG8gd29ybGQ="), base64.urlsafe_b64encode(b"\xfb\xff"), base64.urlsafe_b64decode(b"-_8="))
print(base64.b64encode(b"hi").decode(), base64.b16encode(b"\x01\xab"), base64.b16decode("01AB"), base64.b64decode("YW Jj\nZA=="), base64.encodebytes(b"x" * 60))
print(binascii.hexlify(b"\x00\xff"), binascii.unhexlify("00ff"))
for s in ["abc", "a", "ab=c", "ab==cd==", "@@YWJj", "=Zm9v"]:
    for v in (False, True):
        try:
            print(repr(s), v, base64.b64decode(s, validate=v))
        except binascii.Error as e:
            print(repr(s), v, type(e).__name__, e, isinstance(e, ValueError))
for bad in [b"\xff", b"a\xe2\x82(", b"\xed\xa0", b"ok\xf0\x9f"]:
    try:
        bad.decode()
    except UnicodeDecodeError as e:
        print("decode:", e)
    print(bad.decode("utf-8", "replace"), bad.decode("utf-8", "ignore"))
for f in [lambda: "é€".encode("ascii"), lambda: "a😀".encode("latin-1"), lambda: hashlib.sha256("s"), lambda: b"a" + "b", lambda: bytes([300]), lambda: bytes("x"), lambda: b"ab"[5], lambda: hashlib.new("nope")]:
    try:
        f()
    except (UnicodeEncodeError, TypeError, ValueError, IndexError) as e:
        print(type(e).__name__, e)
