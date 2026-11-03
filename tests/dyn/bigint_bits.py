# Big ints: & | ^ with signs, shifts far past 1M bits, 2 ** n, bin/hex/oct and int(s, base) (found by the speed set).
x = 12345
def r():
    global x
    x = (x * 1103515245 + 12345) % 2147483648
    return x
vals = []
for i in range(40):
    k = r() % 300
    v = (r() << k) + r()
    if r() % 3 == 0:
        v = -v
    vals.append(v)
vals += [0, 1, -1, 2**64, -(2**64), 2**63 - 1, -(2**63)]
acc = 0
for a in vals:
    for b in vals:
        res = [a & b, a | b, a ^ b, a - b, a + b, a * (b % 1000), (a >> (abs(b) % 200)), (a << (abs(b) % 100))]
        if b:
            res += [a // b, a % b]
        for q in res:
            acc = (acc * 31 + q) % (10**18 + 9)
        pass
print(acc)
a = (1 << 5_000_000) + (7 << 3_000_000) + 5
print(a >> 3_000_000 & 15, (a >> 4_999_999), a >> 6_000_000, (-a) >> 6_000_000, (-a) >> 3_000_000 & 15, -5 >> 100, 5 >> 100)
print(2**100, 4**70, 2**0, 8**3, (-2)**65, 2**64 - 1, (2**5000).bit_length())
for v in [0, 1, 5, -5, 255, -256, (1 << 70) + 3, -(1 << 70) - 3, 12345678901234567890123]:
    print(bin(v), oct(v), hex(v), int(bin(v), 0) == v, int(hex(v)[2:] if v >= 0 else hex(v)[3:], 16), f"{v:x}", f"{v:b}"[:12])
print(int("1" * 100, 2) == (1 << 100) - 1, int("-ff" * 10, 16) if False else int("ff" * 10, 16))
g = int("10" * 50000, 2)
print(bin(g).count("1"), len(hex(g)), g.bit_count())
