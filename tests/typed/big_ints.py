def main():
    h = 0
    for c in "the quick brown fox":
        h = h * 31 + ord(c)
    print(h)
    f = 1
    for i in range(1, 30):
        f *= i
    print(f)
    p = 1
    for _ in range(70):
        p = p * 2
    print(p, len("ab"))
main()
a, b = 0, 1
for _ in range(100):
    a, b = b, a + b
print(a)
print(2 ** 40 * 3, 1 << 40)
