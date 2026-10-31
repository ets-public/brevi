import sys
lines = sys.stdin.read().split('\n')
k = lines.index('=====')
def clean(x):
    if x and x[-1] == '': x = x[:-1]
    return x
old = clean(lines[:k]); new = clean(lines[k+1:])
n, m = len(old), len(new)
L = [[0] * (m + 1) for _ in range(n + 1)]
for i in range(n - 1, -1, -1):
    for j in range(m - 1, -1, -1):
        L[i][j] = L[i+1][j+1] + 1 if old[i] == new[j] else max(L[i+1][j], L[i][j+1])
i = j = 0; out = []; a = b = 0
while i < n and j < m:
    if old[i] == new[j]:
        out.append('  ' + old[i]); i += 1; j += 1
    elif L[i+1][j] >= L[i][j+1]:
        out.append('- ' + old[i]); i += 1; a += 1
    else:
        out.append('+ ' + new[j]); j += 1; b += 1
while i < n: out.append('- ' + old[i]); i += 1; a += 1
while j < m: out.append('+ ' + new[j]); j += 1; b += 1
if a == 0 and b == 0: print('no changes')
else:
    out.append('@@ -%d +%d @@' % (a, b)); print('\n'.join(out))
