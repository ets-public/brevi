# Time-limited tasks (set `speed`, written 2026-11-03): ordinary programs whose plain solution does tens of
# millions of simple steps. Every test must finish within e2e.py's 10 second limit. Inputs are a few numbers
# (the data comes from a seeded generator), so the test files stay small while the work is large.
# The question this set answers: does the speed of a Brevi build save an agent the retries (and thinking) it
# spends making CPython code fast enough? Prompts give input sizes, as a real spec would, but no time limit.
# Writes tasks/<name>/{prompt.md,tests/N.in,N.out}.
import os

T = {}


def task(name, prompt, inputs):
    def deco(fn):
        T[name] = (prompt.strip(), inputs, fn)
        return fn
    return deco


RNG = """Random numbers come from this generator: start with `x = SEED`; each next number is `x = (x * 1103515245 + 12345) % 2147483648`, and the number used is the new `x`."""


def rng(seed):
    x = seed
    while True:
        x = (x * 1103515245 + 12345) % 2147483648
        yield x


# ---------------------------------------------------------------- conway
@task("conway", f"""
Simulate Conway's Game of Life on a grid that wraps around at the edges (a torus). Read one line from stdin: `W H G SEED D` (1 <= W, H <= 600, 0 <= G <= 500, 0 <= D <= 100).

{RNG}

The grid has H rows of W cells. Fill it row by row, left to right: a cell starts alive when the next random number `% 100` is less than D.

Each generation, every cell counts its 8 neighbours (wrapping around the edges); a live cell with 2 or 3 live neighbours stays alive, a dead cell with exactly 3 becomes alive, and every other cell is dead in the next generation. All cells change at once.

Print `gen K: N alive` for K = 0 (the start), for every K that is a multiple of 50, and for K = G (once, even if G is a multiple of 50). Then print `checksum S`, where S is the sum of `row * W + column` (both from 0) over the live cells after generation G.
""", ["5 5 4 7 40\n", "1 1 3 1 100\n", "12 7 60 99 35\n", "600 600 500 2026 30\n", "500 550 450 5 45\n"])
def conway(inp):
    W, H, G, seed, D = map(int, inp.split())
    r = rng(seed)
    g = [[1 if next(r) % 100 < D else 0 for _ in range(W)] for _ in range(H)]
    out = []
    def alive():
        return sum(map(sum, g))
    out.append(f"gen 0: {alive()} alive")
    for k in range(1, G + 1):
        n = [[0] * W for _ in range(H)]
        for y in range(H):
            up, row, dn = g[(y - 1) % H], g[y], g[(y + 1) % H]
            nr = n[y]
            for x in range(W):
                l, rr = (x - 1) % W, (x + 1) % W
                c = up[l] + up[x] + up[rr] + row[l] + row[rr] + dn[l] + dn[x] + dn[rr]
                nr[x] = 1 if c == 3 or (c == 2 and row[x]) else 0
        g = n
        if k % 50 == 0 or k == G:
            out.append(f"gen {k}: {alive()} alive")
    s = sum(y * W + x for y in range(H) for x in range(W) if g[y][x])
    out.append(f"checksum {s}")
    return "\n".join(out) + "\n"


# ---------------------------------------------------------------- knapsack
@task("knapsack", f"""
Solve 0/1 knapsack problems. Each line of stdin is one problem: `N CAP SEED` (1 <= N <= 1000, 0 <= CAP <= 300000). Lines that are blank are skipped; any other line that is not three non-negative integers in those ranges prints `line L: bad line` (lines counted from 1) and is skipped.

{RNG}

Item i (from 1 to N) takes two random numbers in turn: its weight is `1 + first % 1000` and its value is `1 + second % 1000`.

Choose a set of items, each used at most once, whose total weight is at most CAP and whose total value is as large as possible. Print `line L: best V weight W`, where V is that largest total value and W is the smallest total weight of any set reaching V.
""", ["3 10 1\n2 1500 2\n\nfoo\n5 0 3\n1 -2 3\n", "4 2000 11\n7 3000 12\n1001 5 5\n", "800 250000 77\n", "1000 300000 4242\n", "900 280000 9\n"])
def knapsack(inp):
    out = []
    for L, line in enumerate(inp.split("\n"), 1):
        if not line.strip():
            continue
        w = line.split()
        if len(w) != 3 or not all(x.isdigit() for x in w):
            out.append(f"line {L}: bad line")
            continue
        n, cap, seed = map(int, w)
        if not (1 <= n <= 1000 and cap <= 300000):
            out.append(f"line {L}: bad line")
            continue
        r = rng(seed)
        items = []
        for _ in range(n):
            a, b = next(r), next(r)
            items.append((1 + a % 1000, 1 + b % 1000))
        # best[c] = (value, -weight) with weight at most c
        NEG = -1
        best = [0] + [NEG] * cap  # best[c] = largest value with total weight exactly c
        for wt, v in items:
            if wt > cap:
                continue
            for c in range(cap, wt - 1, -1):
                p = best[c - wt]
                if p >= 0 and p + v > best[c]:
                    best[c] = p + v
        V = max(best)
        W = best.index(V)
        out.append(f"line {L}: best {V} weight {W}")
    return "\n".join(out) + "\n"


# ---------------------------------------------------------------- edits
@task("edits", f"""
Compute edit distances between generated DNA strings. Each line of stdin is one pair: `LEN_A LEN_B SEED` (0 <= LEN_A, LEN_B <= 10000). Blank lines are skipped; any other line that is not three non-negative integers with lengths in range prints `line L: bad line` (lines counted from 1) and is skipped.

{RNG}

String A takes LEN_A random numbers, then string B takes LEN_B; each number gives the letter `"acgt"[number % 4]`.

Print `line L: distance D same S`, where D is the Levenshtein distance between A and B (the fewest single-letter insertions, deletions and substitutions that turn A into B) and S is the length of their longest common subsequence.
""", ["5 7 3\n0 4 1\n\nx y z\n10001 2 2\n", "40 35 8\n12 12 9\n", "9000 8000 31\n7000 9500 32\n", "10000 10000 2026\n", "8000 8000 7\n1000 1200 8\n"])
def edits(inp):
    out = []
    for L, line in enumerate(inp.split("\n"), 1):
        if not line.strip():
            continue
        w = line.split()
        if len(w) != 3 or not all(x.isdigit() for x in w):
            out.append(f"line {L}: bad line")
            continue
        la, lb, seed = map(int, w)
        if la > 10000 or lb > 10000:
            out.append(f"line {L}: bad line")
            continue
        r = rng(seed)
        a = "".join("acgt"[next(r) % 4] for _ in range(la))
        b = "".join("acgt"[next(r) % 4] for _ in range(lb))
        prev = list(range(lb + 1))
        lp = [0] * (lb + 1)
        for i in range(1, la + 1):
            cur = [i] + [0] * lb
            lc = [0] * (lb + 1)
            ai = a[i - 1]
            for j in range(1, lb + 1):
                if ai == b[j - 1]:
                    cur[j] = prev[j - 1]
                    lc[j] = lp[j - 1] + 1
                else:
                    cur[j] = 1 + min(prev[j - 1], prev[j], cur[j - 1])
                    lc[j] = lp[j] if lp[j] > lc[j - 1] else lc[j - 1]
            prev, lp = cur, lc
        out.append(f"line {L}: distance {prev[lb]} same {lp[lb]}")
    return "\n".join(out) + "\n"


# ---------------------------------------------------------------- chains
@task("chains", """
Find long Collatz chains. The chain of a positive integer n repeats `n = n / 2` (n even) or `n = 3n + 1` (n odd) until n is 1; its length counts the numbers in it, both ends included (the chain of 1 has length 1, of 6 has length 9).

Each line of stdin is a query `A B` (1 <= A <= B <= 3000000). Blank lines are skipped; any other line that is not two integers in range with A <= B prints `line L: bad query` (lines counted from 1) and is skipped.

For each query print `A..B: N has length K`, where N is the number in A..B with the longest chain (the smallest such number on a tie) and K its length. At the end print `Q queries`, counting the queries answered.
""", ["1 10\n6 6\n\n10 1\n0 5\nfoo bar\n", "1 1000\n500 600\n27 27\n", "1 3000000\n", "1000000 2999999\n1 2000000\n", "2500000 3000000\n123 456789\n"])
def chains(inp):
    LIM = 3000001
    memo = [0] * LIM
    memo[1] = 1
    def length(n):
        path = []
        m = n
        while m >= LIM or memo[m] == 0:
            path.append(m)
            m = m // 2 if m % 2 == 0 else 3 * m + 1
        k = memo[m]
        for p in reversed(path):
            k += 1
            if p < LIM:
                memo[p] = k
        return memo[n]
    out, q = [], 0
    for L, line in enumerate(inp.split("\n"), 1):
        if not line.strip():
            continue
        w = line.split()
        ok = len(w) == 2 and all(x.isdigit() for x in w)
        if ok:
            a, b = map(int, w)
            ok = 1 <= a <= b <= 3000000
        if not ok:
            out.append(f"line {L}: bad query")
            continue
        bn, bk = a, 0
        for n in range(a, b + 1):
            k = length(n)
            if k > bk:
                bn, bk = n, k
        out.append(f"{a}..{b}: {bn} has length {bk}")
        q += 1
    out.append(f"{q} queries")
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    import sys
    only = sys.argv[1:]
    for name, (prompt, inputs, fn) in T.items():
        if only and name not in only:
            continue
        d = f"tasks/{name}"
        os.makedirs(f"{d}/tests", exist_ok=True)
        open(f"{d}/prompt.md", "w").write(prompt + "\n")
        for i, inp in enumerate(inputs):
            open(f"{d}/tests/{i}.in", "w").write(inp)
            open(f"{d}/tests/{i}.out", "w").write(fn(inp))
        print(name, "done", flush=True)
