def fannkuch(n):
    perm1 = list(range(n))
    count = [0] * n
    max_flips = 0
    checksum = 0
    r = n
    nperm = 0
    while True:
        while r != 1:
            count[r - 1] = r
            r -= 1
        perm = perm1[:]
        flips = 0
        k = perm[0]
        while k:
            perm[:k + 1] = perm[k::-1]
            flips += 1
            k = perm[0]
        if flips > max_flips:
            max_flips = flips
        checksum += flips if nperm % 2 == 0 else -flips
        while True:
            if r == n:
                return checksum, max_flips
            p0 = perm1[0]
            for i in range(r):
                perm1[i] = perm1[i + 1]
            perm1[r] = p0
            count[r] -= 1
            if count[r] > 0:
                break
            r += 1
        nperm += 1

n = int(input())
c, m = fannkuch(n)
print(c)
print(f"Pfannkuchen({n}) = {m}")
