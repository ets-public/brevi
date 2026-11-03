n = int(input())
a = [(i * 31 + 7) % 26 for i in range(n)]
b = [(i * 17 + 3) % 26 for i in range(n)]
prev = [0] * (n + 1)
for i in range(1, n + 1):
    cur = [0] * (n + 1)
    ai = a[i - 1]
    for j in range(1, n + 1):
        if ai == b[j - 1]:
            cur[j] = prev[j - 1] + 1
        else:
            cur[j] = cur[j - 1] if cur[j - 1] > prev[j] else prev[j]
    prev = cur
print(prev[n])
