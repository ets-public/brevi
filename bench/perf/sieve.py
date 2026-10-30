n = int(input())
p = [True]*(n+1)
i = 2
while i*i <= n:
    if p[i]:
        for j in range(i*i, n+1, i): p[j] = False
    i += 1
print(sum(1 for k in range(2, n+1) if p[k]))
