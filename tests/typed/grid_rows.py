# `nrow = new[r]` then `nrow[c] = ...` in a loop: written back once after the loop (was a row copy per cell).
W, H = 6, 4
grid = [[(r * 7 + c * 3) % 2 for c in range(W)] for r in range(H)]
for gen in range(3):
    new = [[0] * W for _ in range(H)]
    for r in range(H):
        up, row, dn = grid[(r - 1) % H], grid[r], grid[(r + 1) % H]
        nrow = new[r]
        for c in range(W):
            n = up[(c - 1) % W] + up[c] + up[(c + 1) % W] + row[(c - 1) % W] + row[(c + 1) % W] + dn[(c - 1) % W] + dn[c] + dn[(c + 1) % W]
            nrow[c] = 1 if n == 3 or (n == 2 and row[c]) else 0
    grid = new
    print(gen, [sum(r) for r in grid])
# The row is read through the container inside the loop: still written back every time.
t = [[0, 0, 0]]
row = t[0]
for i in range(3):
    row[i] = i + 1
    print(t[0][:i + 1])
