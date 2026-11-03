def popcount(x):
    return bin(x).count("1")
def tok(s):
    out = []
    i = 0
    while i < len(s):
        if s.startswith("<->", i):
            out.append("iff")
            i += 3
        elif s.endswith("x", i):
            out.append("x-end")
            i += 1
        else:
            out.append(s[i])
            i += 1
    return out
print(popcount(255), bin(5), bin(-5), hex(255), hex(-1), oct(8), bin(0), bin(True))
print(tok("a<->bx"), "abc".startswith("", 3), "abc".startswith("", 4), "abc".startswith("c", -1), "héllo".startswith("llo", 2))
