import sys

data = sys.stdin.read().split("\n")
k = data.index("=====") if "=====" in data else len(data)
def lines(chunk):
    if chunk and chunk[-1] == "":
        chunk = chunk[:-1]
    return chunk
old = lines(data[:k])
new = lines(data[k + 1:])
n, m = len(old), len(new)
L = [[0] * (m + 1) for _ in range(n + 1)]
for i in range(n - 1, -1, -1):
    for j in range(m - 1, -1, -1):
        if old[i] == new[j]:
            L[i][j] = L[i + 1][j + 1] + 1
        else:
            L[i][j] = max(L[i + 1][j], L[i][j + 1])
out = []
i = j = 0
a = b = 0
while i < n and j < m:
    if old[i] == new[j]:
        out.append("  " + old[i])
        i += 1
        j += 1
    elif L[i + 1][j] >= L[i][j + 1]:
        out.append("- " + old[i])
        i += 1
        a += 1
    else:
        out.append("+ " + new[j])
        j += 1
        b += 1
while i < n:
    out.append("- " + old[i]); i += 1; a += 1
while j < m:
    out.append("+ " + new[j]); j += 1; b += 1
if a == 0 and b == 0:
    print("no changes")
else:
    out.append("@@ -%d +%d @@" % (a, b))
    print("\n".join(out))
