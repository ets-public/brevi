import sys


def main():
    text = sys.stdin.read()
    lines = text.split("\n")
    k = lines.index("=====")
    old = lines[:k]
    rest = lines[k + 1:]
    if rest and rest[-1] == "":
        rest = rest[:-1]
    new = rest

    if old == new:
        sys.stdout.write("no changes\n")
        return

    n = len(old)
    m = len(new)
    # L[i][j] = LCS length of old[i:] and new[j:]
    L = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(n - 1, -1, -1):
        for j in range(m - 1, -1, -1):
            if old[i] == new[j]:
                L[i][j] = L[i + 1][j + 1] + 1
            else:
                L[i][j] = max(L[i + 1][j], L[i][j + 1])

    out = []
    removed = 0
    added = 0
    i = 0
    j = 0
    while i < n and j < m:
        if old[i] == new[j]:
            out.append("  " + old[i])
            i += 1
            j += 1
        elif L[i + 1][j] >= L[i][j + 1]:
            out.append("- " + old[i])
            removed += 1
            i += 1
        else:
            out.append("+ " + new[j])
            added += 1
            j += 1
    while i < n:
        out.append("- " + old[i])
        removed += 1
        i += 1
    while j < m:
        out.append("+ " + new[j])
        added += 1
        j += 1

    out.append("@@ -%d +%d @@" % (removed, added))
    sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
