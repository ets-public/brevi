def rolling(s):
    MOD = (1 << 61) - 1
    h = 0
    for c in s:
        h = (h * 1000003 + ord(c)) % MOD
    return h
print(rolling("hello world, this is brevi"))
