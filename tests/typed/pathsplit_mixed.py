# Typed-build compile failures from the path-split case study (bench/pathsplit): each must build typed and print
# what CPython prints.
from decimal import Decimal


def rle_decode(code):
    out = []
    i = 0
    while i < len(code):
        ch = code[i]
        i += 1
        j = i
        while j < len(code) and code[j].isdigit():
            j += 1
        if j > i:
            out.append(ch * int(code[i:j]))
        i = j
    return "".join(out)


def rep(s, k):
    ch = s[0]
    return ch * int(s[1:]) + k * ch + ch * -1


RATES = {'gold': Decimal('0.10'), 'silver': Decimal('0.05'), 'basic': Decimal('0')}
ZERO = Decimal('0')


def disc(t):
    return RATES.get(t, ZERO) * 100


for i in range(1, 16):
    print("FizzBuzz" if i % 15 == 0 else "Fizz" if i % 3 == 0 else "Buzz" if i % 5 == 0 else i)
    print(i > 3 if i % 2 else 2.5, "x" if i % 4 else True)
print(rle_decode("a3b1c12"), rep("z3", 2))
d = {}
f = {}
for k, v in [("a", "1.50"), ("b", "2"), ("a", "0.25"), ("c", "-0.0")]:
    d[k] = d.get(k, 0) + Decimal(v)
    f[k] = f.get(k, 0) + float(v)
    f[k] = f.get(k, 1) * 2.0
print(d, f)
print(disc('gold'), disc('none'), 'gold' in RATES)
for line in ["  put a  b c  ", "get x", "", "   ", "one", "a\x1fb c", "\x1c x \x1d"]:
    print(line.split(None, 1), line.split(None, 0), line.split(None, 2), line.split(None), line.split())
    print(repr(line.strip()), repr(line.lstrip()), repr(line.rstrip()), line.isspace())
print(Decimal("\x1f 4.20 \x1e"))
