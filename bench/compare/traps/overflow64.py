n = int(input())
x = 9223372036854775807
print(x + n)
print(-x - 1 - n)
print(x * n)
h = 0
for c in "the quick brown fox":
    h = h * 31 + ord(c)
print(h)
