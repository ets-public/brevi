# Release suite tasks (written 2026-11-03, never used to tune Brevi). Eighteen held-out tasks of mixed size and kind,
# added to the six xl3 tasks to make the release test's 24-task set `rel` (see subject_prompt_sets.py).
# The mix is meant to look like what agents write: small scripts, medium tools that lean on the standard library
# (json, csv, fractions, decimal, datetime, re, heapq, itertools), simulations with classes, and three multi-file programs.
# Writes tasks/<name>/{prompt.md,tests/N.in,N.out} and, for multi-file tasks, tasks/<name>/files.
# usage: python3 ref/gen_rel.py   (from bench/)
import os, re, sys, json, copy, heapq, random, datetime, itertools
from fractions import Fraction
from decimal import Decimal, ROUND_HALF_UP

T = {}
FILES = {}
def task(name, prompt, inputs, files=None):
    def deco(fn):
        T[name] = (prompt.strip(), inputs, fn)
        if files: FILES[name] = files
        return fn
    return deco

def lines_of(s):
    ls = s.split("\n")
    if ls and ls[-1] == "": ls.pop()
    return ls
def d2(x): return str(Decimal(x).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))

MULTI = """
Write the program as {n} files in the folder you are given: {files}. `main` is the entry point; each file imports what it needs from the others. {roles}
"""

# ---------------------------------------------------------------- bowling (small)
@task("bowling", r"""
Score ten-pin bowling games. Each non-blank line of stdin is one game: a player name, then the number of pins knocked down by each roll, separated by spaces. Lines are numbered from 1; blank lines are ignored.

Rules: a game has 10 frames. In frames 1 to 9, a roll of 10 is a strike and ends the frame; otherwise the frame has two rolls whose sum is at most 10 (a spare if it is exactly 10). A strike scores 10 plus the next two rolls; a spare scores 10 plus the next roll; any other frame scores its pins. In frame 10, a strike earns two bonus rolls and a spare earns one, all thrown in frame 10; frame 10 scores the sum of its rolls. After a strike in frame 10, if the first bonus roll is not a strike, the two bonus rolls add up to at most 10.

For each game print one line:
- `NAME: SCORE` for a complete game.
- `NAME: invalid` if a roll is not a whole number from 0 to 10, a frame has too many pins, or there are rolls after the game is over.
- `NAME: incomplete after frame K, score S` if the rolls run out first. K is the number of frames fully rolled. S adds up the completed frames in order and stops at the first one whose bonus rolls have not all been thrown yet.

Check the rolls in order: the first problem found decides between invalid and incomplete. A line with a name and no rolls is `incomplete after frame 0, score 0`.

At the end print `best: NAME SCORE` for the highest complete game (the first one in the input on a tie), or `best: none` if no game is complete.
""", [
"""ann 10 10 10 10 10 10 10 10 10 10 10 10
bob 9 1 9 1 9 1 9 1 9 1 9 1 9 1 9 1 9 1 9 1 9
cid 3 4 3 4 3 4 3 4 3 4 3 4 3 4 3 4 3 4 3 4
dee 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
""",
"""ann 10 7 3 9 0 10 0 8 8 2 0 6 10 10 10 8 1
bob 10 10 10
cid 5 5 5
dee 6 5 1
eve 10 10 10 10 10 10 10 10 10 10 10 10 10
fay 3 4 x 3
gus
hal 1 2 3 11

ida 10 10 10 10 10 10 10 10 10 10 5 6
""",
"""zed 0 10 10 0 5 5 5 5 1 1 10 10 10 0 0 1 9 10 3
yan 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 9 1 10
xia 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 10 10 10
wes 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 10 4 5
vic 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 10 4 6
uma 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 10 10 4
tom 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1
sal 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 5 5
""",
"""max 10 10 10 10 10 10 10 10 10 10 10 9
amy 10 10 10 10 10 10 10 10 10 10 9 1
leo 10 5
kim 10 5 4 10
jon 2 8 10 3
ina 2 -1
""",
])
def _(s):
    out, best = [], None
    for line in lines_of(s):
        w = line.split()
        if not w: continue
        name, toks = w[0], w[1:]
        r = []
        bad = False
        for t in toks:
            if not re.fullmatch(r"\d+", t) or int(t) > 10: bad = True; break
            r.append(int(t))
        def run():
            if bad: return ("invalid",)
            n, i, frames = len(r), 0, []
            for f in range(1, 11):
                if i >= n: return ("inc", frames)
                if f < 10:
                    if r[i] == 10: frames.append((i, "X")); i += 1; continue
                    if i + 1 >= n: return ("inc", frames)
                    if r[i] + r[i+1] > 10: return ("invalid",)
                    frames.append((i, "/" if r[i] + r[i+1] == 10 else "o")); i += 2
                else:
                    a = r[i]
                    if i + 1 >= n: return ("inc", frames)
                    b = r[i+1]
                    if a == 10:
                        if i + 2 >= n: return ("inc", frames)
                        c = r[i+2]
                        if b < 10 and b + c > 10: return ("invalid",)
                        frames.append((i, "T3")); i += 3
                    elif a + b > 10: return ("invalid",)
                    elif a + b == 10:
                        if i + 2 >= n: return ("inc", frames)
                        frames.append((i, "T3")); i += 3
                    else:
                        frames.append((i, "T2")); i += 2
            if i < n: return ("invalid",)
            return ("ok", frames)
        res = run()
        if res[0] == "invalid": out.append(f"{name}: invalid"); continue
        total = 0
        for (i, k) in res[1]:
            need = {"X": 3, "/": 3, "o": 2, "T3": 3, "T2": 2}[k]
            if i + need > len(r): break
            total += sum(r[i:i+need])
        if res[0] == "inc":
            out.append(f"{name}: incomplete after frame {len(res[1])}, score {total}")
        else:
            out.append(f"{name}: {total}")
            if best is None or total > best[1]: best = (name, total)
    out.append(f"best: {best[0]} {best[1]}" if best else "best: none")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- text_table (small)
@task("text_table", r"""
Turn comma-separated rows from stdin into a Markdown-style text table. Lines are numbered from 1. The first non-blank line is the header; every later non-blank line is a data row. Blank lines are ignored. Cells are split on `,` (there is no quoting) and have spaces stripped from both ends.

- A data row with fewer cells than the header is padded with empty cells. A data row with more cells is left out of the table, and after the table the program prints `skipped line N: K cells` for each one, in input order.
- A column is numeric when it has at least one non-empty cell and every non-empty cell matches `-?[0-9]+(\.[0-9]+)?`. In a numeric column every non-empty cell is shown with as many decimal places as the cell with the most decimals in that column (pad with zeros: `2` becomes `2.00` if another cell is `1.25`).
- If at least one column is numeric, the table ends with a totals row: each numeric column shows the sum of its cells, with the column's decimal places; the first column shows `total` unless it is numeric; other text columns are empty.

Print the header row, then a separator row, then the data rows in input order, then the totals row if any. Each row is `| ` + the cells joined by ` | ` + ` |`. Each column is as wide as its widest entry among the header, the shown cells and the totals row (including the word `total`). Text cells are padded on the right, numeric cells on the left; header names are padded on the right in every column. The separator has, for each column, `-` repeated (width + 2) times for a text column, or `-` repeated (width + 1) times followed by `:` for a numeric column, joined by `|`, with `|` at both ends.

Finish with `R rows, C columns` (R = data rows shown, C = header cells). If the input has no header, print only `0 rows, 0 columns`.
""", [
"""name,qty,price
apple,3,0.5
banana,12,0.25
cherry,100,2
""",
"""city , country, pop, note
Oslo, NO, 709000, capital
Bergen,NO,285900
Lyon ,FR, 522250 , big, extra
Nice, FR, , sunny

Turku,FI,-12.5,odd
""",
"""id,score
1,10
2,9.5
3,
4,7.25
""",
"""

word,len,code
a,1,x1
bb,2,2
ccc,,3
""",
])
def _(s):
    rows, skipped = [], []
    header = None
    for n, line in enumerate(lines_of(s), 1):
        if not line.strip(): continue
        cells = [c.strip() for c in line.split(",")]
        if header is None: header = cells; continue
        if len(cells) > len(header): skipped.append((n, len(cells))); continue
        rows.append(cells + [""] * (len(header) - len(cells)))
    if header is None: return "0 rows, 0 columns\n"
    C = len(header)
    num, dec = [], []
    for j in range(C):
        vals = [r[j] for r in rows if r[j] != ""]
        isnum = bool(vals) and all(re.fullmatch(r"-?[0-9]+(\.[0-9]+)?", v) for v in vals)
        num.append(isnum)
        dec.append(max((len(v.split(".")[1]) if "." in v else 0) for v in vals) if isnum else 0)
    def fmt(v, j):
        if not num[j] or v == "": return v
        q = Decimal(1).scaleb(-dec[j])
        return str(Decimal(v).quantize(q))
    shown = [[fmt(r[j], j) for j in range(C)] for r in rows]
    tot = None
    if any(num):
        tot = []
        for j in range(C):
            if num[j]:
                sm = sum((Decimal(r[j]) for r in rows if r[j] != ""), Decimal(0))
                tot.append(str(sm.quantize(Decimal(1).scaleb(-dec[j]))))
            else:
                tot.append("total" if j == 0 else "")
    allr = [header] + shown + ([tot] if tot else [])
    w = [max(len(r[j]) for r in allr) for j in range(C)]
    def row(cells, hdr=False):
        return "| " + " | ".join(c.ljust(w[j]) if hdr or not num[j] else c.rjust(w[j]) for j, c in enumerate(cells)) + " |"
    out = [row(header, True), "|" + "|".join(("-" * (w[j] + 1) + ":") if num[j] else "-" * (w[j] + 2) for j in range(C)) + "|"]
    out += [row(r) for r in shown]
    if tot: out.append(row(tot))
    for n, k in skipped: out.append(f"skipped line {n}: {k} cells")
    out.append(f"{len(rows)} rows, {C} columns")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- huffman (small)
@task("huffman", r"""
Build a Huffman code for each line of stdin (lines numbered from 1, without the newline; every character counts, spaces included).

Start with one leaf per distinct character, weighted by how often it appears. Repeatedly remove the two nodes with the smallest weight and join them under a new node whose weight is their sum, until one node is left. Ties are broken by the smallest character in the node (a leaf's character, or the smallest character among an inner node's leaves); the first node removed becomes the left child (bit `0`), the second the right child (bit `1`). A character's code is the path from the root to its leaf. If a line has only one distinct character, its code is `0`.

For each line print `line N: K symbols, B bits, ratio R`, where K is the number of distinct characters, B the length of the whole line encoded, and R is B divided by (8 times the number of characters in the line), with 3 decimal places. Then one line per character, sorted by character: `'C' COUNT CODE`. An empty line prints `line N: empty`.
""", [
"""abracadabra
aaaa
mississippi river
""",
"""the quick brown fox jumps over the lazy dog

ab
""",
"""  x  y  z
AAAAABBBBCCCDDE
""",
"""zzzzzzzzzzzzzzzzyyyyyyyyxxxxwwv
1+1=2, 2+2=4!
""",
])
def _(s):
    out = []
    for n, line in enumerate(lines_of(s), 1):
        if line == "": out.append(f"line {n}: empty"); continue
        cnt = {}
        for ch in line: cnt[ch] = cnt.get(ch, 0) + 1
        codes = {}
        if len(cnt) == 1:
            codes[line[0]] = "0"
        else:
            h = [(w, ch, ch) for ch, w in cnt.items()]
            heapq.heapify(h)
            while len(h) > 1:
                w1, m1, t1 = heapq.heappop(h)
                w2, m2, t2 = heapq.heappop(h)
                heapq.heappush(h, (w1 + w2, min(m1, m2), (t1, t2)))
            def walk(t, p):
                if isinstance(t, str): codes[t] = p
                else: walk(t[0], p + "0"); walk(t[1], p + "1")
            walk(h[0][2], "")
        bits = sum(len(codes[ch]) for ch in line)
        out.append(f"line {n}: {len(cnt)} symbols, {bits} bits, ratio {bits / (8 * len(line)):.3f}")
        for ch in sorted(cnt): out.append(f"'{ch}' {cnt[ch]} {codes[ch]}")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- units (small)
@task("units", r"""
Convert between units from facts given on stdin. Lines are numbered from 1; blank lines are ignored. Unit names are one or more ASCII letters and are case-sensitive. NUMBER below is digits with an optional `.` and more digits (`3`, `0.001`).

- `1 A = X B` says one A is X B (X is a NUMBER greater than 0, and A and B are different units). Conversions work both ways and chain through other units. If A and B can already be converted through earlier facts, the fact is ignored and the program prints `line N: already known` if it gives the same factor (exactly) or `line N: conflict` otherwise.
- `? Q A in B` converts the NUMBER Q of unit A to unit B and prints `Q A = V B`, with Q exactly as written and V formatted as Python's format spec `.6g` prints the float of the exact result. If A or B never appeared in an accepted fact print `unknown unit U` (checking A first). If both are known but there is no chain from A to B, print `cannot convert A to B`. A unit converts to itself with factor 1.
- Anything else prints `line N: bad line`.

Work out conversion factors exactly (as fractions) and turn the result into a float only to print it.
""", [
"""1 km = 1000 m
1 m = 100 cm
1 inch = 2.54 cm
1 ft = 12 inch
? 3 km in cm
? 1 ft in m
? 10 m in ft
? 0.5 inch in km
""",
"""1 h = 60 min
1 min = 60 s
1 day = 24 h
1 day = 86400 s
1 day = 86000 s
? 1 day in s
? 90 min in h
? 1 s in day
? 2 kg in g
1 kg = 1000 g
? 2 kg in s
? 2 kg in g
1 kg = 0 g
1 kg is 1000 g
? 3 day in day
""",
"""1 a = 3 b
1 c = 5 b
1 d = 0.2 c
1 a = 0.6 d
1 d = 1 b
? 1 a in d
? 7 d in a

? 1000000 a in b
? 123456789 b in a
1 KM = 1000 m
? 1 KM in m
? 1 Km in m
""",
"""1 lb = 16 oz
1 stone = 14 lb
1 kg = 2.20462 lb
? 1 stone in kg
? 100 oz in kg
? 1 oz in stone
1 oz = 28.3495 g
? 1 kg in g
1 g = 0.001 kg
? 1 g in kg
""",
])
def _(s):
    g = {}
    def path(a, b):
        if a not in g or b not in g: return None
        seen = {a: Fraction(1)}; q = [a]
        while q:
            u = q.pop(0)
            for v, f in g[u]:
                if v not in seen: seen[v] = seen[u] * f; q.append(v)
        return seen.get(b)
    out = []
    num = r"\d+(\.\d+)?"
    for n, line in enumerate(lines_of(s), 1):
        w = line.split()
        if not w: continue
        if len(w) == 5 and w[0] == "1" and w[2] == "=" and w[1] != w[4] and re.fullmatch(r"[A-Za-z]+", w[1]) and re.fullmatch(r"[A-Za-z]+", w[4]) and re.fullmatch(num, w[3]) and Fraction(w[3]) > 0:
            a, x, b = w[1], Fraction(w[3]), w[4]
            known = path(a, b)
            if known is not None:
                out.append(f"line {n}: already known" if known == x else f"line {n}: conflict"); continue
            g.setdefault(a, []).append((b, x)); g.setdefault(b, []).append((a, 1 / x))
        elif len(w) == 5 and w[0] == "?" and w[3] == "in" and re.fullmatch(num, w[1]) and re.fullmatch(r"[A-Za-z]+", w[2]) and re.fullmatch(r"[A-Za-z]+", w[4]):
            a, b = w[2], w[4]
            if a not in g: out.append(f"unknown unit {a}"); continue
            if b not in g: out.append(f"unknown unit {b}"); continue
            f = path(a, b)
            if f is None: out.append(f"cannot convert {a} to {b}"); continue
            out.append(f"{w[1]} {a} = {float(Fraction(w[1]) * f):.6g} {b}")
        else:
            out.append(f"line {n}: bad line")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- life (small, some speed)
def _life_grid(seed, W, H, p):
    rnd = random.Random(seed)
    return "\n".join("".join("#" if rnd.random() < p else "." for _ in range(W)) for _ in range(H))

@task("life", r"""
Run Conway's Game of Life on a grid that wraps around at the edges (the left edge touches the right, the top touches the bottom). The first line of stdin is `W H G`: the width, height and number of generations. The next H lines have W characters each: `#` for a live cell, `.` for a dead one.

Each generation, every cell counts its live neighbours among the 8 around it (with wrapping). A live cell with 2 or 3 live neighbours stays alive; a dead cell with exactly 3 becomes alive; every other cell is dead in the next generation.

Run up to G generations. The starting grid is generation 0. If generation T is the same as an earlier generation S, stop there and print `generation T repeats generation S` (use the earliest such S). Then print the grid you stopped at (generation G if nothing repeated) in the same format as the input, then `population P`, the number of live cells in it.
""", [
"""5 5 4
.....
..#..
..#..
..#..
.....
""",
"""6 6 10
......
.##...
.##...
...##.
...##.
......
""",
"""8 6 7
.#......
..#.....
###.....
........
........
........
""",
"48 48 120\n" + _life_grid(7, 48, 48, 0.35) + "\n",
])
def _(s):
    ls = lines_of(s)
    W, H, G = map(int, ls[0].split())
    cur = tuple(ls[1:1 + H])
    seen = {cur: 0}
    msg = None
    for t in range(1, G + 1):
        nxt = []
        for y in range(H):
            row = []
            for x in range(W):
                c = 0
                for dy in (-1, 0, 1):
                    for dx in (-1, 0, 1):
                        if (dy or dx) and cur[(y + dy) % H][(x + dx) % W] == "#": c += 1
                alive = cur[y][x] == "#"
                row.append("#" if (c == 3 or (alive and c == 2)) else ".")
            nxt.append("".join(row))
        cur = tuple(nxt)
        if cur in seen: msg = f"generation {t} repeats generation {seen[cur]}"; break
        seen[cur] = t
    out = ([msg] if msg else []) + list(cur) + [f"population {sum(r.count('#') for r in cur)}"]
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- wordle (small)
@task("wordle", r"""
Score games of a five-letter word game. Each non-blank line of stdin is one game: the answer, then up to 6 guesses, separated by spaces, optionally with the word `HARD` before the answer. Lines are numbered from 1; blank lines are ignored.

A guess is scored letter by letter: `G` if the letter is in the same place in the answer; otherwise `Y` if the letter appears in the answer at a place not already marked `G` and not already used for an earlier `Y` of the same letter (looking at the guess from left to right); otherwise `.`.

Check each line in this order and print the first problem found, as `line N: ...`, instead of scoring that game:
1. `bad word W`: the first word (answer or guess) that is not exactly 5 lowercase letters a to z.
2. `too many guesses` if there are more than 6 guesses.
3. `guess after solve` if there is a guess after one that matched the answer.
4. In HARD mode, `guess K breaks hard mode` (K counts guesses from 1) if a guess does not keep every `G` letter from the earlier guesses in its place, or does not contain each letter at least as many times as it was marked `G` or `Y` in any single earlier guess.

Otherwise print `ANSWER: P1 P2 ... solved in K` (the patterns of all guesses) or `ANSWER: P1 P2 ... not solved` (also when there are no guesses: `ANSWER: not solved`). Lines with a problem are not counted below.

At the end print `played P, solved S, average A`, where A is the average number of guesses over solved games with 2 decimals, or `-` if none was solved, then `1:C1 2:C2 3:C3 4:C4 5:C5 6:C6` counting solved games by number of guesses.
""", [
"""crane slate trace crane
apple paper plead apple
eerie eager there erode
hello world
""",
"""HARD crane slate grace crane
HARD crane slate stone
HARD sassy essay sassy
HARD rebus bring rubes rebus
abbey kebab abbey
""",
"""speed abide speed spend
speed Abide
speed crane crate crash
speed a b c d e f g
llama allay llama
llama lllll
""",
"""oxbow
robot bloom trouble
HARD robot booth robot
HARD robot torso motor robot
geese eagle eerie geese
""",
])
def _(s):
    out = []
    played = solved = 0; dist = [0] * 7
    def score(ans, g):
        res = ["."] * 5; left = {}
        for i in range(5):
            if g[i] == ans[i]: res[i] = "G"
            else: left[ans[i]] = left.get(ans[i], 0) + 1
        for i in range(5):
            if res[i] != "G" and left.get(g[i], 0) > 0: res[i] = "Y"; left[g[i]] -= 1
        return "".join(res)
    for n, line in enumerate(lines_of(s), 1):
        w = line.split()
        if not w: continue
        hard = w[0] == "HARD"
        if hard: w = w[1:]
        if not w: out.append(f"line {n}: bad word "); continue
        bad = next((x for x in w if not re.fullmatch(r"[a-z]{5}", x)), None)
        if bad is not None: out.append(f"line {n}: bad word {bad}"); continue
        ans, gs = w[0], w[1:]
        if len(gs) > 6: out.append(f"line {n}: too many guesses"); continue
        if any(g == ans for g in gs[:-1]): out.append(f"line {n}: guess after solve"); continue
        pats = [score(ans, g) for g in gs]
        prob = None
        if hard:
            for k in range(1, len(gs)):
                g = gs[k]
                for j in range(k):
                    p, pg = pats[j], gs[j]
                    if any(p[i] == "G" and g[i] != pg[i] for i in range(5)): prob = k + 1; break
                    need = {}
                    for i in range(5):
                        if p[i] in "GY": need[pg[i]] = need.get(pg[i], 0) + 1
                    if any(g.count(c) < m for c, m in need.items()): prob = k + 1; break
                if prob: break
        if prob: out.append(f"line {n}: guess {prob} breaks hard mode"); continue
        played += 1
        body = " ".join(pats)
        if gs and gs[-1] == ans:
            solved += 1; dist[len(gs)] += 1
            out.append(f"{ans}: {body} solved in {len(gs)}")
        else:
            out.append(f"{ans}: {body} not solved" if body else f"{ans}: not solved")
    avg = f"{sum(i * dist[i] for i in range(7)) / solved:.2f}" if solved else "-"
    out.append(f"played {played}, solved {solved}, average {avg}")
    out.append(" ".join(f"{i}:{dist[i]}" for i in range(1, 7)))
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- json_patch (medium)
@task("json_patch", r"""
Apply edits to a JSON document. The first line of stdin is a JSON document on one line. Every later line (numbered from 2, blank lines ignored) is an operation:

- `add PATH VALUE`, `replace PATH VALUE`, `test PATH VALUE`: VALUE is the rest of the line after PATH, a JSON text (it may contain spaces).
- `remove PATH`, `print PATH`
- `move FROM PATH`, `copy FROM PATH`

A PATH starts with `/`. The path `/` alone means the whole document. Otherwise it is split on `/` into keys after the first `/`; in a key, `~1` stands for `/` and `~0` for `~`. In an object a key names a member; in an array it must be an index written as a whole number without leading zeros (`0`, `7`), or `-` in the cases below.

What each operation does:
- `add`: the container the last key points into must exist. In an object, set the member (adding or overwriting it). In an array, the key is an index from 0 to the array's length, and the value is inserted there; `-` appends. Adding at `/` replaces the whole document.
- `remove`: the value must exist; remove it (later array items move down). Removing `/` is an error.
- `replace`: the value must exist; replace it (at `/`, the whole document).
- `move`: the value at FROM must exist, and PATH may not be inside FROM (PATH starts with FROM followed by `/`). Remove it from FROM, then `add` it at PATH. Moving to the same path changes nothing.
- `copy`: like move, but FROM keeps its value and PATH may be inside FROM; PATH gets an independent copy.
- `test`: the value must exist and equal VALUE, or the program prints `line N: test failed`. Equal means the same JSON type with equal contents, except that numbers compare by value (`1` equals `1.0`); `true` is not equal to `1`. Object member order does not matter.
- `print`: print the value at PATH as compact JSON (below), or `line N: error` if it does not exist.

A line that is not one of these forms, with a bad PATH (not starting with `/`) or a VALUE that is not valid JSON, prints `line N: bad line`. An operation whose paths do not work as described prints `line N: error` and changes nothing.

At the end print the document. Compact JSON is what Python's `json.dumps(value, sort_keys=True, separators=(",", ":"))` prints.
""", [
"""{"name": "app", "tags": ["a", "b"], "conf": {"debug": false, "level": 3}}
add /tags/1 "x"
add /tags/- "z"
replace /conf/level 4
remove /tags/0
print /tags
add /conf/paths {"home": "/h", "tmp": "/t"}
move /conf/paths/tmp /temp
copy /conf /conf2
replace /conf2/debug true
print /conf
test /conf/level 4.0
test /conf/debug 0
print /missing
""",
"""[1, 2, 3]
add /3 4
add /5 6
add /01 9
remove /-
add / {"a/b": 1, "c~d": [true, null]}
print /a~1b
replace /c~0d/1 "set"
remove /c~0d/0
print /c~0d
remove /
move /c~0d /c~0d/0
move /a~1b /a~1b
copy /c~0d /c~0d/0
print /

oops /a
add a 1
add /x {bad json}
""",
"""{"users": [{"id": 1, "roles": ["admin"]}, {"id": 2, "roles": []}], "count": 2}
copy /users/0 /users/-
replace /users/2/id 3
add /users/2/roles/0 "owner"
print /users/0/roles
test /users/0 {"roles": ["admin"], "id": 1}
test /users/2 {"roles": ["owner", "admin"], "id": 3.0}
test /count "2"
replace /count 3
move /users/1 /users/0
print /users
remove /users/5
replace /users/2/name "x"
add /users/2/name "x"
test /users/2/name "x"
""",
"""{"a": {"b": {"c": [1, {"d": 2}]}}, "e": 1.5, "f": "~/x"}
print /a/b/c/1/d
move /a/b /e
print /e
add /e/c/1/d/x 1
add /e/c/1/e [1, 2, [3]]
remove /e/c/0
test /e/c/0/e/2 [3]
replace / [{"z": 0}]
print /0/z
add /0/z/q 1
print /f
""",
])
def _(s):
    ls = lines_of(s)
    doc = [json.loads(ls[0])]   # doc[0] is the document
    out = []
    def keys_of(p):
        if not p.startswith("/"): return None
        if p == "/": return []
        return [k.replace("~1", "/").replace("~0", "~") for k in p[1:].split("/")]
    def idx(k, n, allow_end):
        if allow_end and k == "-": return n
        if not re.fullmatch(r"0|[1-9][0-9]*", k): return None
        i = int(k)
        return i if i < n + (1 if allow_end else 0) else None
    def get(ks):
        cur = doc[0]
        for k in ks:
            if isinstance(cur, dict):
                if k not in cur: raise KeyError
                cur = cur[k]
            elif isinstance(cur, list):
                i = idx(k, len(cur), False)
                if i is None: raise KeyError
                cur = cur[i]
            else: raise KeyError
        return cur
    def add(ks, v):
        if not ks: doc[0] = v; return
        par = get(ks[:-1]); k = ks[-1]
        if isinstance(par, dict): par[k] = v
        elif isinstance(par, list):
            i = idx(k, len(par), True)
            if i is None: raise KeyError
            par.insert(i, v)
        else: raise KeyError
    def remove(ks):
        if not ks: raise KeyError
        par = get(ks[:-1]); k = ks[-1]
        if isinstance(par, dict):
            if k not in par: raise KeyError
            return par.pop(k)
        if isinstance(par, list):
            i = idx(k, len(par), False)
            if i is None: raise KeyError
            return par.pop(i)
        raise KeyError
    def eq(a, b):
        if isinstance(a, bool) or isinstance(b, bool): return type(a) is type(b) and a == b
        if isinstance(a, (int, float)) and isinstance(b, (int, float)): return a == b
        if isinstance(a, dict) and isinstance(b, dict): return a.keys() == b.keys() and all(eq(a[k], b[k]) for k in a)
        if isinstance(a, list) and isinstance(b, list): return len(a) == len(b) and all(eq(x, y) for x, y in zip(a, b))
        return type(a) is type(b) and a == b
    dump = lambda v: json.dumps(v, sort_keys=True, separators=(",", ":"))
    for n, line in enumerate(ls[1:], 2):
        if not line.strip(): continue
        w = line.split(None, 2)
        op = w[0]
        try:
            if op in ("add", "replace", "test") and len(w) == 3:
                ks = keys_of(w[1]); v = json.loads(w[2])
                if ks is None: raise ValueError
            elif op in ("remove", "print") and len(w) == 2:
                ks = keys_of(w[1])
                if ks is None: raise ValueError
            elif op in ("move", "copy") and len(w) == 3 and len(w[2].split()) == 1:
                ks0, ks = keys_of(w[1]), keys_of(w[2])
                if ks0 is None or ks is None: raise ValueError
            else: raise ValueError
        except ValueError:
            out.append(f"line {n}: bad line"); continue
        saved = copy.deepcopy(doc[0])
        try:
            if op == "add": add(ks, v)
            elif op == "replace":
                get(ks)
                if not ks: doc[0] = v
                else:
                    par = get(ks[:-1])
                    if isinstance(par, dict): par[ks[-1]] = v
                    else: par[idx(ks[-1], len(par), False)] = v
            elif op == "test":
                if not eq(get(ks), v): out.append(f"line {n}: test failed")
            elif op == "remove": remove(ks)
            elif op == "print": out.append(dump(get(ks)))
            elif op == "move":
                if ks0 == ks: get(ks0)
                else:
                    if ks[:len(ks0)] == ks0 and len(ks) > len(ks0): raise KeyError
                    get(ks0); v = remove(ks0); add(ks, v)
            elif op == "copy":
                v = copy.deepcopy(get(ks0)); add(ks, v)
        except (KeyError, TypeError, IndexError):
            doc[0] = saved
            out.append(f"line {n}: error")
    out.append(dump(doc[0]))
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- csv_join (medium)
@task("csv_join", r"""
Read two CSV tables and answer queries about them. Stdin has three parts separated by lines that are exactly `---`: table `a`, table `b`, then queries. Each table starts with a header row. Rows are read as Python's `csv` module reads them (fields may be quoted with `"`, contain commas, and use `""` for a quote); no field contains a line break. Lines are numbered from 1 over the whole input. Blank lines are ignored everywhere.

When a table row has a different number of fields from its header, the row is dropped; before answering any query, print `line N: wrong field count` for each dropped row in input order.

Queries, one per line:
- `join KEY`: an inner join on column KEY, which both tables must have. The result has a's columns, then b's columns except KEY; a b column whose name is already used gets `_b` added. For each a row in order, one result row per b row with the same KEY value, in b's order.
- `left KEY`: the same, but an a row with no match gives one row with empty b fields.
- `count T COL`: for table T (`a` or `b`), the distinct values of COL with how many rows have each, as rows `value,count` under the header `COL,count`, by count (largest first) then value.
- `sum T COL BY`: for table T, adds up COL for each distinct value of BY, as rows `value,total` under the header `BY,COL`, sorted by value. Every COL value in T must be a decimal number (`-?[0-9]+(\.[0-9]+)?`); totals have as many decimal places as the most precise value of COL in the table.

Print each result as CSV the way Python's `csv.writer` writes it with `lineterminator="\n"` (fields quoted only when needed), header row first, then a line `(R rows)`.

Errors print one line instead of a result: `line N: unknown table T`, `line N: unknown column C` (the first column named in the query that the table lacks; for joins, check a first, then b), `line N: COL is not numeric`, or `line N: bad query` for anything else.
""", [
"""id,name,city
1,Ann,Oslo
2,Bob,"Bergen, west"
3,Cid,Oslo
4,Dee,Turku
---
id,amount,city
1,10.5,Oslo
3,2,Oslo
1,4.25,Bergen
5,1,Nowhere
---
join id
left id
count a city
sum b amount id
""",
"""sku,desc,price
A1,"Widget, small",2.50
A2,"Gadget ""Pro""\",10
A3,Thing,1.255
A4,Broken
---
sku,qty
A1,3
A2,1
A1,2
A9,7
,1
---
join sku
count b sku
sum a price sku
sum b qty sku
sum a desc sku
join qty
count c sku
count a nope
select *
left
""",
"""k,v
x,1
y,2
x,3

---
k,v,w
x,10,a
z,30,b
x,20,c
---
join k
left k
count a k
sum a v k
sum b v w
""",
"""name,team,points
"Lee, A",red,10
Kim,blue,-3.5
Ola,red,7
Pat,green,
Uma,blue,2
---
team,coach
red,"Sam ""Coach"" Hill"
blue,Joe
---
left team
join team
count a team
sum a points team
""",
])
def _(s):
    import csv, io
    ls = lines_of(s)
    parts, cur = [], []
    for n, line in enumerate(ls, 1):
        if line == "---" and len(parts) < 2: parts.append(cur); cur = []
        else: cur.append((n, line))
    parts.append(cur)
    out = []
    tabs = {}
    for name, part in zip("ab", parts[:2]):
        hdr, rows = None, []
        for n, line in part:
            if not line.strip(): continue
            row = next(csv.reader([line]))
            if hdr is None: hdr = row; continue
            if len(row) != len(hdr): out.append(f"line {n}: wrong field count"); continue
            rows.append(row)
        tabs[name] = (hdr, rows)
    def emit(hdr, rows):
        buf = io.StringIO(); wr = csv.writer(buf, lineterminator="\n")
        wr.writerow(hdr)
        for r in rows: wr.writerow(r)
        out.append(buf.getvalue().rstrip("\n")); out.append(f"({len(rows)} rows)")
    NUM = r"-?[0-9]+(\.[0-9]+)?"
    for n, line in parts[2]:
        w = line.split()
        if not w: continue
        if w[0] in ("join", "left") and len(w) == 2:
            k = w[1]; (ha, ra), (hb, rb) = tabs["a"], tabs["b"]
            if k not in ha or k not in hb: out.append(f"line {n}: unknown column {k}"); continue
            ia, ib = ha.index(k), hb.index(k)
            keep = [j for j in range(len(hb)) if j != ib]
            names = list(ha)
            for j in keep:
                nm = hb[j]
                names.append(nm + "_b" if nm in names else nm)
            res = []
            for r in ra:
                ms = [x for x in rb if x[ib] == r[ia]]
                for m in ms: res.append(r + [m[j] for j in keep])
                if not ms and w[0] == "left": res.append(r + [""] * len(keep))
            emit(names, res)
        elif w[0] in ("count", "sum") and len(w) == (3 if w[0] == "count" else 4):
            if w[1] not in tabs: out.append(f"line {n}: unknown table {w[1]}"); continue
            h, rows = tabs[w[1]]
            miss = next((c for c in w[2:] if c not in h), None)
            if miss: out.append(f"line {n}: unknown column {miss}"); continue
            if w[0] == "count":
                j = h.index(w[2]); c = {}
                for r in rows: c[r[j]] = c.get(r[j], 0) + 1
                emit([w[2], "count"], [[v, str(k)] for v, k in sorted(c.items(), key=lambda t: (-t[1], t[0]))])
            else:
                j, b = h.index(w[2]), h.index(w[3])
                vals = [r[j] for r in rows]
                if not all(re.fullmatch(NUM, v) for v in vals): out.append(f"line {n}: {w[2]} is not numeric"); continue
                dp = max([len(v.split(".")[1]) if "." in v else 0 for v in vals] or [0])
                tot = {}
                for r in rows: tot[r[b]] = tot.get(r[b], Decimal(0)) + Decimal(r[j])
                emit([w[3], w[2]], [[v, str(t.quantize(Decimal(1).scaleb(-dp)))] for v, t in sorted(tot.items())])
        else:
            out.append(f"line {n}: bad query")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- matrix (medium)
@task("matrix", r"""
Write a calculator for matrices of exact fractions. Each line of stdin is a command; lines are numbered from 1 and blank lines are ignored. Names are a letter followed by letters or digits.

- `NAME = [ROW; ROW; ...]` defines a matrix. A ROW is numbers separated by spaces; every row must have the same number of entries, at least one. A number is an integer (`-3`), a fraction of integers (`2/3`, `-1/4`; not divided by zero) or a decimal (`0.5`, `-1.25`).
- `NAME = X + Y`, `NAME = X - Y`, `NAME = X * Y` (matrix product), `NAME = X'` (transpose), `NAME = inv X` (inverse), `NAME = X ^ K` (X multiplied by itself K times, K a whole number of 0 or more; `X ^ 0` is the identity), where X and Y are defined names. Operators and names are separated by spaces, except `'`, which follows the name directly.
- `print X` prints `X = [ROW; ROW; ...]` in the same form, with each entry as a reduced fraction (`3`, `-1/2`, `0`).
- `det X` prints `det X = D`; `rank X` prints `rank X = R`.

Errors print one line and change nothing: `line N: unknown NAME` for an undefined name (the first one in the line), `line N: size mismatch` for sizes that do not fit (+ and - need the same size, X * Y needs X's column count to equal Y's row count), `line N: not square` when inv, det or ^ gets a non-square matrix, `line N: singular` when inv gets a matrix whose determinant is 0, and `line N: bad line` for anything else (including a matrix literal with uneven rows or a bad number).
""", [
"""A = [1 2; 3 4]
B = [0 1; 1 0]
C = A * B
print C
D = A + B
print D
det A
I = inv A
print I
E = A * I
print E
F = A'
print F
rank A
""",
"""M = [1/2 0.25 -1; 2 -3/4 0]
print M
N = M'
P = M * N
print P
det P
Q = inv P
print Q
det M
R = M * M
R = M + N
S = X + M
rank M
K = [1 2 3; 2 4 6; 1 1 1]
rank K
det K
L = inv K
""",
"""A = [2 0; 0 2]
B = A ^ 0
print B
C = A ^ 5
print C
D = [1 1; 1 0]
F = D ^ 10
print F
G = D ^ -1
H = [1 2; 3]
J = [1 2/0]
Z = [0 0; 0 0]
rank Z
W = inv Z
print W
print Q9
x1 = [7]
y = inv x1
print y
det x1
""",
"""T = [1 2 3; 0 1 4; 5 6 0]
U = inv T
print U
V = U * T
print V
det T
det U
W = T - T
print W
rank W
X = [1.5 -2.25]
Y = X'
Z = X * Y
print Z
Z2 = Y * X
print Z2
rank Z2
""",
])
def _(s):
    env, out = {}, []
    NAME = r"[A-Za-z][A-Za-z0-9]*"
    def num(t):
        if re.fullmatch(r"-?\d+", t): return Fraction(int(t))
        m = re.fullmatch(r"(-?\d+)/(-?\d+)", t)
        if m:
            if int(m.group(2)) == 0: return None
            return Fraction(int(m.group(1)), int(m.group(2)))
        if re.fullmatch(r"-?\d+\.\d+", t): return Fraction(t)
        return None
    def fmt(M): return "[" + "; ".join(" ".join(str(x) for x in r) for r in M) + "]"
    def mul(X, Y): return [[sum((X[i][k] * Y[k][j] for k in range(len(Y))), Fraction(0)) for j in range(len(Y[0]))] for i in range(len(X))]
    def ident(n): return [[Fraction(int(i == j)) for j in range(n)] for i in range(n)]
    def elim(M):
        A = [r[:] for r in M]; rows, cols = len(A), len(A[0]); det = Fraction(1); r = 0
        for c in range(cols):
            p = next((i for i in range(r, rows) if A[i][c] != 0), None)
            if p is None: det = Fraction(0); continue
            if p != r: A[r], A[p] = A[p], A[r]; det = -det
            det *= A[r][c]
            for i in range(rows):
                if i != r and A[i][c] != 0:
                    f = A[i][c] / A[r][c]
                    A[i] = [a - f * b for a, b in zip(A[i], A[r])]
            r += 1
            if r == rows: break
        return r, det
    def inv(M):
        n = len(M); A = [r[:] + e for r, e in zip(M, ident(n))]
        for c in range(n):
            p = next(i for i in range(c, n) if A[i][c] != 0)
            A[c], A[p] = A[p], A[c]
            pv = A[c][c]; A[c] = [x / pv for x in A[c]]
            for i in range(n):
                if i != c and A[i][c] != 0:
                    f = A[i][c]; A[i] = [a - f * b for a, b in zip(A[i], A[c])]
        return [r[n:] for r in A]
    class Err(Exception): pass
    for n, line in enumerate(lines_of(s), 1):
        if not line.strip(): continue
        w = line.split()
        try:
            def look(x):
                if x not in env: raise Err(f"unknown {x}")
                return env[x]
            if len(w) >= 3 and re.fullmatch(NAME, w[0]) and w[1] == "=":
                rhs = line.split("=", 1)[1].strip()
                if rhs.startswith("[") and rhs.endswith("]"):
                    rows = [r.split() for r in rhs[1:-1].split(";")]
                    if not rows or not rows[0] or any(len(r) != len(rows[0]) for r in rows): raise Err("bad line")
                    M = [[num(t) for t in r] for r in rows]
                    if any(x is None for r in M for x in r): raise Err("bad line")
                    env[w[0]] = M; continue
                t = rhs.split()
                if len(t) == 3 and t[1] in "+-*" and re.fullmatch(NAME, t[0]) and re.fullmatch(NAME, t[2]):
                    X, Y = look(t[0]), look(t[2])
                    if t[1] == "*":
                        if len(X[0]) != len(Y): raise Err("size mismatch")
                        env[w[0]] = mul(X, Y)
                    else:
                        if len(X) != len(Y) or len(X[0]) != len(Y[0]): raise Err("size mismatch")
                        sg = 1 if t[1] == "+" else -1
                        env[w[0]] = [[a + sg * b for a, b in zip(r, q)] for r, q in zip(X, Y)]
                elif len(t) == 1 and t[0].endswith("'") and re.fullmatch(NAME, t[0][:-1]):
                    X = look(t[0][:-1]); env[w[0]] = [list(c) for c in zip(*X)]
                elif len(t) == 2 and t[0] == "inv" and re.fullmatch(NAME, t[1]):
                    X = look(t[1])
                    if len(X) != len(X[0]): raise Err("not square")
                    if elim(X)[1] == 0: raise Err("singular")
                    env[w[0]] = inv(X)
                elif len(t) == 3 and t[1] == "^" and re.fullmatch(NAME, t[0]) and re.fullmatch(r"\d+", t[2]):
                    X = look(t[0])
                    if len(X) != len(X[0]): raise Err("not square")
                    R = ident(len(X))
                    for _ in range(int(t[2])): R = mul(R, X)
                    env[w[0]] = R
                else: raise Err("bad line")
            elif len(w) == 2 and w[0] in ("print", "det", "rank") and re.fullmatch(NAME, w[1]):
                X = look(w[1])
                if w[0] == "print": out.append(f"{w[1]} = {fmt(X)}")
                elif w[0] == "rank": out.append(f"rank {w[1]} = {elim(X)[0]}")
                else:
                    if len(X) != len(X[0]): raise Err("not square")
                    out.append(f"det {w[1]} = {elim(X)[1]}")
            else: raise Err("bad line")
        except Err as e:
            out.append(f"line {n}: {e}")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- router (medium)
@task("router", r"""
Write a URL router. Lines of stdin are numbered from 1; blank lines are ignored.

- `ROUTE METHOD PATTERN NAME` adds a route. METHOD is one or more uppercase letters. PATTERN starts with `/` and is split on `/` into segments after the first `/` (the pattern `/` alone has no segments). Each segment is a literal (letters, digits, `-`, `_` and `.`), `<name>` (matches any one segment), `<int:name>` (matches one segment of digits only) or `<path:name>` (matches one or more segments, joined back with `/`; only allowed as the last segment). Parameter names are letters, digits and `_`, starting with a letter or `_`, and may not repeat within a pattern. A route with a pattern that breaks these rules prints `line N: bad route`. A route with the same METHOD and the same pattern shape as an earlier one (the same segments, with parameter names ignored) prints `line N: duplicate route`. Both are ignored.
- `METHOD PATH` is a request. PATH starts with `/` and is split the same way; if any segment is empty (as in `//` or a trailing `/`, except for the path `/` itself) print `400 PATH`.

To answer a request, find the routes whose pattern matches the path. If none do, print `404 PATH`. Otherwise keep those with the request's method; a HEAD request with no matching HEAD route uses the matching GET routes instead. If none are left, print `405 PATH allow: M1,M2,...` with the distinct methods of the matching routes, sorted. Otherwise pick the most specific route: give each segment of a pattern a rank (literal 0, int 1, plain 2, path 3) and compare the lists of ranks like words in a dictionary (the smaller list wins; a list that is a prefix of the other is smaller). On a tie the route added first wins. Print `200 NAME` followed by ` name=value` for each parameter in pattern order (an int parameter printed as a number without leading zeros), and then ` (head)` if a HEAD request was answered by a GET route.

Anything else prints `line N: bad line`. At the end print `R routes, Q requests, K ok` (routes accepted, requests answered, requests answered with 200).
""", [
"""ROUTE GET / home
ROUTE GET /users users_list
ROUTE POST /users users_new
ROUTE GET /users/<int:id> user_show
ROUTE GET /users/me me
ROUTE GET /users/<name> user_by_name
ROUTE DELETE /users/<int:id> user_delete
GET /
GET /users
GET /users/42
GET /users/007
GET /users/me
GET /users/bob
DELETE /users/bob
PUT /users
GET /posts
HEAD /users/5
GET /users/
""",
"""ROUTE GET /files/<path:rest> files
ROUTE GET /files/<name>/raw raw
ROUTE GET /files/readme.md readme
ROUTE GET /files/<path:p>/x bad
ROUTE GET /a/<x>/<x> bad2
ROUTE GET /a/<int:> bad3
ROUTE GET /files/<other> one_file
ROUTE GET /files/<path:all> dup
ROUTE HEAD /files/<path:rest> head_files
GET /files/readme.md
GET /files/notes.txt
GET /files/docs/a/b.txt
GET /files/x/raw
HEAD /files/x/raw
HEAD /files/x
GET /files
GET //files
ROUTE GET nopath bad5
FETCH
""",
"""ROUTE GET /shop/<cat>/<int:id> item
ROUTE GET /shop/<cat>/<slug> item_slug
ROUTE GET /shop/sale/<int:id> sale_item
ROUTE PUT /shop/<c>/<int:i> item_put
ROUTE GET /shop/<path:p> shop_any
ROUTE PATCH /shop/sale/<int:id> sale_patch
GET /shop/toys/12
GET /shop/toys/robot
GET /shop/sale/0012
GET /shop/sale/x
PUT /shop/sale/3
PATCH /shop/toys/3
PATCH /shop/sale/3
GET /shop/a/b/c
POST /shop/a/b/c
HEAD /shop/sale/1
ROUTE GET /shop/<c2>/<int:i2> dup_item
""",
"""ROUTE GET /v1/<_ver>/x v
ROUTE GET /v1/<9a>/x bad
ROUTE POST /api/<path:rest> api
ROUTE GET /api/status status
ROUTE GET /api/<int:n>/status nstatus
GET /api/status
POST /api/status
DELETE /api/status
GET /api/12/status
POST /api/12/status/
GET /v1/abc/x
GET /v1/abc/x/y
ROUTE GET /dot.ted/a-b_c dotted
GET /dot.ted/a-b_c
ROUTE GET /bad seg/x bad6
""",
])
def _(s):
    routes, out = [], []
    nroutes = nreq = nok = 0
    LIT = r"[A-Za-z0-9._-]+"; PNAME = r"[A-Za-z_][A-Za-z0-9_]*"
    def segs(p): return [] if p == "/" else p[1:].split("/")
    def parse(p):
        if not p.startswith("/"): return None
        ss, shape, names = segs(p), [], []
        for i, sg in enumerate(ss):
            m = re.fullmatch(r"<(?:(int|path):)?(" + PNAME + r")>", sg)
            if m:
                kind = {"int": 1, None: 2, "path": 3}[m.group(1)]
                if kind == 3 and i != len(ss) - 1: return None
                if m.group(2) in names: return None
                names.append(m.group(2)); shape.append((kind, None))
            elif re.fullmatch(LIT, sg): shape.append((0, sg))
            else: return None
        return shape, names
    def match(shape, names, ps):
        vals = []
        for i, (k, lit) in enumerate(shape):
            if k == 3:
                if i >= len(ps): return None
                vals.append("/".join(ps[i:])); return vals
            if i >= len(ps): return None
            if k == 0 and ps[i] != lit: return None
            if k == 1:
                if not re.fullmatch(r"[0-9]+", ps[i]): return None
                vals.append(str(int(ps[i])))
            if k == 2: vals.append(ps[i])
        return vals if len(ps) == len(shape) else None
    for n, line in enumerate(lines_of(s), 1):
        w = line.split()
        if not w: continue
        if w[0] == "ROUTE" and len(w) == 4 and re.fullmatch(r"[A-Z]+", w[1]):
            r = parse(w[2])
            if r is None: out.append(f"line {n}: bad route"); continue
            if any(m == w[1] and sh == r[0] for m, sh, _, _ in routes): out.append(f"line {n}: duplicate route"); continue
            routes.append((w[1], r[0], r[1], w[3])); nroutes += 1
        elif len(w) == 2 and re.fullmatch(r"[A-Z]+", w[0]) and w[0] != "ROUTE" and w[1].startswith("/"):
            nreq += 1; p = w[1]; ps = segs(p)
            if any(x == "" for x in ps): out.append(f"400 {p}"); continue
            hits = [(rt, match(rt[1], rt[2], ps)) for rt in routes]
            hits = [(rt, v) for rt, v in hits if v is not None]
            if not hits: out.append(f"404 {p}"); continue
            mine = [(rt, v) for rt, v in hits if rt[0] == w[0]]
            head = False
            if not mine and w[0] == "HEAD":
                mine = [(rt, v) for rt, v in hits if rt[0] == "GET"]; head = bool(mine)
            if not mine: out.append(f"405 {p} allow: " + ",".join(sorted({rt[0] for rt, _ in hits}))); continue
            best = min(range(len(mine)), key=lambda i: ([k for k, _ in mine[i][0][1]], i))
            rt, v = mine[best]
            out.append(f"200 {rt[3]}" + "".join(f" {nm}={x}" for nm, x in zip(rt[2], v)) + (" (head)" if head else "")); nok += 1
        else:
            out.append(f"line {n}: bad line")
    out.append(f"{nroutes} routes, {nreq} requests, {nok} ok")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- sudoku (medium, some speed)
_SOL = ["534678912", "672195348", "198342567", "859761423", "426853791", "713924856", "961537284", "287419635", "345286179"]
def _count(g, limit=2):
    # number of solutions of grid g (lists of ints, 0 = empty), up to limit
    def ok(r, c, d):
        if d in g[r] or any(g[i][c] == d for i in range(9)): return False
        br, bc = r - r % 3, c - c % 3
        return all(g[i][j] != d for i in range(br, br + 3) for j in range(bc, bc + 3))
    n = [0]
    def rec():
        best = None
        for r in range(9):
            for c in range(9):
                if g[r][c] == 0:
                    cand = [d for d in range(1, 10) if ok(r, c, d)]
                    if best is None or len(cand) < len(best[2]): best = (r, c, cand)
        if best is None: n[0] += 1; return
        r, c, cand = best
        for d in cand:
            g[r][c] = d; rec(); g[r][c] = 0
            if n[0] >= limit: return
    rec(); return n[0]
def _holes(seed, k, unique=True):
    # a puzzle with k empty cells made from _SOL (relabelled and shuffled by seed); unique=True keeps one solution
    rnd = random.Random(seed)
    perm = list("123456789"); rnd.shuffle(perm)
    g = [[int(perm[int(ch) - 1]) for ch in r] for r in _SOL]
    cells = [(r, c) for r in range(9) for c in range(9)]; rnd.shuffle(cells)
    blanks = 0
    for r, c in cells:
        if blanks == k: break
        v = g[r][c]; g[r][c] = 0
        if unique and _count([row[:] for row in g]) != 1: g[r][c] = v; continue
        blanks += 1
    return "\n".join("".join(str(x) if x else "." for x in r) for r in g)
@task("sudoku", r"""
Solve sudoku puzzles. Stdin holds one or more puzzles separated by blank lines (several blank lines count as one; blank lines at the start or end are ignored). A puzzle is 9 lines of 9 characters: a digit `1` to `9` for a given cell, or `.` for an empty one. Puzzles are numbered from 1.

For each puzzle print:
- `puzzle K: bad input` if it is not 9 lines of 9 such characters.
- `puzzle K: invalid` if two givens with the same digit share a row, a column or a 3x3 box.
- `puzzle K: no solution` if the empty cells cannot be filled so that every row, column and 3x3 box holds the digits 1 to 9.
- `puzzle K: multiple solutions` if there is more than one way.
- otherwise `puzzle K: solved`, then the 9 lines of the solved grid.
""", [
_holes(1, 40) + "\n\n" + _holes(2, 45) + "\n",
_holes(3, 50) + "\n\n\n" + "\n".join(["53..7....", "6..195...", ".98....6.", "8...6...3", "4..8.3..1", "7...2...6", ".6....28.", "...419..5", "....8..79"]) + "\n",
"\n" + "\n".join(["12345678.", "........9"] + ["........."] * 7) + "\n\n" + "\n".join(["55.......", "........."] + ["........."] * 7) + "\n\n" + "\n".join(["123", "456"]) + "\n\n" + _holes(4, 58, unique=False) + "\n",
_holes(5, 30) + "\n\n" + "\n".join(["0" * 9] * 9) + "\n\n" + _holes(6, 52, unique=False) + "\n",
])
def _(s):
    blocks, cur = [], []
    for line in lines_of(s):
        if line.strip() == "":
            if cur: blocks.append(cur); cur = []
        else: cur.append(line)
    if cur: blocks.append(cur)
    out = []
    for k, b in enumerate(blocks, 1):
        if len(b) != 9 or any(not re.fullmatch(r"[1-9.]{9}", r) for r in b): out.append(f"puzzle {k}: bad input"); continue
        g = [[0 if ch == "." else int(ch) for ch in r] for r in b]
        def ok(r, c, d):
            for i in range(9):
                if (i != c and g[r][i] == d) or (i != r and g[i][c] == d): return False
            br, bc = r - r % 3, c - c % 3
            return not any(g[i][j] == d and (i, j) != (r, c) for i in range(br, br + 3) for j in range(bc, bc + 3))
        if any(g[r][c] and not ok(r, c, g[r][c]) for r in range(9) for c in range(9)): out.append(f"puzzle {k}: invalid"); continue
        sols = []
        def rec():
            best = None
            for r in range(9):
                for c in range(9):
                    if g[r][c] == 0:
                        cand = [d for d in range(1, 10) if ok(r, c, d)]
                        if best is None or len(cand) < len(best[2]): best = (r, c, cand)
            if best is None: sols.append([row[:] for row in g]); return
            r, c, cand = best
            for d in cand:
                g[r][c] = d; rec(); g[r][c] = 0
                if len(sols) > 1: return
        rec()
        if not sols: out.append(f"puzzle {k}: no solution")
        elif len(sols) > 1: out.append(f"puzzle {k}: multiple solutions")
        else: out.append(f"puzzle {k}: solved"); out += ["".join(map(str, r)) for r in sols[0]]
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- tellers (medium, simulation)
@task("tellers", r"""
Simulate a bank with several tellers. Lines of stdin are numbered from 1; blank lines are ignored. The first non-blank line is `TELLERS K` (K at least 1; otherwise print `line N: bad line` and stop). Every later line is a customer: `NAME ARRIVAL SERVICE` with optional words after it, each `vip` or `patience=P`. ARRIVAL is a whole number of minutes from 0, SERVICE and P whole numbers from 1. A line with a different shape or a bad value prints `line N: bad line`; a NAME already used prints `line N: duplicate`; both are ignored. Customers may be listed in any order of arrival.

Tellers are numbered 1 to K. Time moves in whole minutes. At each minute T, in this order:
1. Tellers whose customer is done at T become free.
2. Customers arriving at T join the queue.
3. While a teller is free and the queue is not empty, the lowest-numbered free teller takes the first customer in the queue and prints `T NAME starts at teller X, waited W`. The queue puts vip customers before the others; within each group, earlier arrival first, then the order of the input lines.
4. A customer with patience P who is still waiting at minute ARRIVAL + P leaves and prints `T NAME leaves` (in queue order if several leave at once).

A customer served from minute S is done at S + SERVICE. When everyone has been served or has left, print `served S, left L, average wait A` (A = the average wait of served customers, 2 decimals, or `-` if none), then `teller X: busy B` for each teller (B = total minutes serving), then `done at T`, the last minute a customer was done (0 if none).
""", [
"""TELLERS 2
ann 0 5
bob 1 3
cid 2 4
dee 2 2 vip
eve 3 1
""",
"""TELLERS 1
amy 0 10
ben 1 2 patience=5
cal 2 3 patience=8
dan 4 1 vip patience=3
eli 9 2 patience=1
fay 10 1
""",
"""

TELLERS 3
zoe 5 2
yan 0 7 vip
xia 0 7
wes 0 7
vic 0 3 patience=2
uma 0 1 vip
tom 6 x
sal 6 2 patience=0
ron 6 2 fast
zoe 7 1
quinn 7 2 vip patience=10
""",
"""TELLERS 2
p1 0 4 patience=4
p2 0 4 patience=4
p3 0 4 patience=4
p4 0 4 patience=4
p5 0 4 patience=8
p6 4 1 vip patience=0
""",
])
def _(s):
    out = []
    ls = [(n, l.split()) for n, l in enumerate(lines_of(s), 1) if l.strip()]
    if not ls: return "\n"
    n0, w0 = ls[0]
    if not (len(w0) == 2 and w0[0] == "TELLERS" and re.fullmatch(r"\d+", w0[1]) and int(w0[1]) >= 1):
        return f"line {n0}: bad line\n"
    K = int(w0[1]); cust, names = [], set()
    for idx, (n, w) in enumerate(ls[1:]):
        good = len(w) >= 3 and re.fullmatch(r"\d+", w[1]) and re.fullmatch(r"\d+", w[2]) and int(w[2]) >= 1
        vip, pat = False, None
        if good:
            for x in w[3:]:
                m = re.fullmatch(r"patience=(\d+)", x)
                if x == "vip": vip = True
                elif m and int(m.group(1)) >= 1: pat = int(m.group(1))
                else: good = False
        if not good: out.append(f"line {n}: bad line"); continue
        if w[0] in names: out.append(f"line {n}: duplicate"); continue
        names.add(w[0]); cust.append({"name": w[0], "arr": int(w[1]), "svc": int(w[2]), "vip": vip, "pat": pat, "i": idx})
    free_at = [0] * (K + 1); busy = [0] * (K + 1); serving = [None] * (K + 1)
    pending = sorted(cust, key=lambda c: (c["arr"], c["i"]))
    queue, served, left, waits, done = [], 0, 0, [], 0
    t = 0
    while pending or queue or any(serving[1:]):
        for x in range(1, K + 1):
            if serving[x] and free_at[x] == t: serving[x] = None
        while pending and pending[0]["arr"] == t: queue.append(pending.pop(0))
        queue.sort(key=lambda c: (not c["vip"], c["arr"], c["i"]))
        for x in range(1, K + 1):
            if serving[x] is None and queue:
                c = queue.pop(0); serving[x] = c; free_at[x] = t + c["svc"]; busy[x] += c["svc"]
                done = max(done, t + c["svc"]); served += 1; waits.append(t - c["arr"])
                out.append(f"{t} {c['name']} starts at teller {x}, waited {t - c['arr']}")
        for c in list(queue):
            if c["pat"] is not None and c["arr"] + c["pat"] == t:
                queue.remove(c); left += 1; out.append(f"{t} {c['name']} leaves")
        t += 1
    out.append(f"served {served}, left {left}, average wait " + (f"{sum(waits) / served:.2f}" if served else "-"))
    for x in range(1, K + 1): out.append(f"teller {x}: busy {busy[x]}")
    out.append(f"done at {done}")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- cron_next (medium, dates)
@task("cron_next", r"""
Work out when cron schedules run next. The first line of stdin is `NOW YYYY-MM-DD HH:MM`. Every later non-blank line is `NAME MIN HOUR DAY MONTH WEEKDAY`: a name and five fields.

Each field is a comma-separated list of items. An item is `*` (every value), a number `V`, a range `A-B` (A at most B), or `*` or a range followed by `/S` (every S-th value of it, starting from its first value; S at least 1). Allowed values: minute 0 to 59, hour 0 to 23, day of month 1 to 31, month 1 to 12, weekday 0 to 7, where 0 and 7 both mean Sunday and 1 is Monday. A value outside its range, or an item that does not fit these forms, makes the field bad.

A minute matches when its minute, hour and month are in their fields and its day matches: if both the DAY and WEEKDAY fields are exactly `*`, every day matches; if only one of them is `*`, the other must contain the day; if neither is `*`, the day matches when either field contains it.

For each schedule print `NAME: T1, T2, T3`: the first three matching minutes strictly after NOW, as `YYYY-MM-DD HH:MM`. Only look up to 2 years after NOW (up to and including the same date and time two years later); if fewer than three are found, print the ones found followed by `, no more` (`NAME: T1, no more`), or `NAME: never` if none. A line with the wrong number of fields prints `NAME: bad line`; otherwise if a field is bad print `NAME: bad field K` with K the number of the first bad field (1 to 5).
""", [
"""NOW 2026-11-03 07:34
every * * * * *
hourly 0 * * * *
morning 30 9 * * 1-5
twice 0,30 12 * * *
monthly 15 3 1 * *
""",
"""NOW 2027-06-01 00:00
leap 0 12 29 2 *
never 0 0 30 2 *
fri13 0 0 13 * 5
sundays 0 6 * * 0
sevens 59 23 * * 7
steps */20 */6 * * *
quarter 0 0 1 */3 *
""",
"""NOW 2026-12-31 23:59
ny 0 0 1 1 *
range 5-10/2 8 * * *
mixed 0 8 1,15 * 1
bad1 60 * * * *
bad2 * 24 * * *
bad3 * * 0 * *
bad4 * * * 13 *
bad5 * * * * 8
short * * *
step0 */0 * * * *
odd 1-0 * * * *
junk a * * * *
""",
"""NOW 2028-02-28 22:15
end 0 0 31 * *
feb 0 23 28-29 2 *
wkend 45 22 * * 6,0
year 0 0 1 1 1
listy 1,2,3 0 1 3 *
""",
])
def _(s):
    ls = [l for l in lines_of(s) if l.strip()]
    now = datetime.datetime.strptime(ls[0][4:].strip(), "%Y-%m-%d %H:%M")
    end = now.replace(year=now.year + 2) if not (now.month == 2 and now.day == 29) else now.replace(year=now.year + 2, day=28)
    out = []
    def field(txt, lo, hi):
        vals = set()
        for it in txt.split(","):
            m = re.fullmatch(r"(\*|(\d+)-(\d+)|(\d+))(?:/(\d+))?", it)
            if not m or (m.group(4) is not None and m.group(5) is not None): return None
            if m.group(1) == "*": a, b = lo, hi
            elif m.group(4) is not None: a = b = int(m.group(4))
            else: a, b = int(m.group(2)), int(m.group(3))
            st = int(m.group(5)) if m.group(5) is not None else 1
            if a < lo or b > hi or a > b or st < 1: return None
            vals |= set(range(a, b + 1, st))
        return vals
    for line in ls[1:]:
        w = line.split()
        if len(w) != 6: out.append(f"{w[0]}: bad line"); continue
        specs = [(0, 59), (0, 23), (1, 31), (1, 12), (0, 7)]
        fs = [field(w[i + 1], *specs[i]) for i in range(5)]
        badk = next((i + 1 for i in range(5) if fs[i] is None), None)
        if badk: out.append(f"{w[0]}: bad field {badk}"); continue
        mi, hr, dom, mon, dow = fs
        dow = {d % 7 for d in dow}
        dstar, wstar = w[3] == "*", w[5] == "*"
        def day_ok(d):
            wd = (d.weekday() + 1) % 7
            if dstar and wstar: return True
            if dstar: return wd in dow
            if wstar: return d.day in dom
            return d.day in dom or wd in dow
        found = []
        day = now.date()
        while day <= end.date() and len(found) < 3:
            if day.month in mon and day_ok(day):
                for h in sorted(hr):
                    for m in sorted(mi):
                        t = datetime.datetime(day.year, day.month, day.day, h, m)
                        if now < t <= end and len(found) < 3: found.append(t)
            day += datetime.timedelta(days=1)
        ts = [t.strftime("%Y-%m-%d %H:%M") for t in found]
        if not ts: out.append(f"{w[0]}: never")
        elif len(ts) < 3: out.append(f"{w[0]}: " + ", ".join(ts) + ", no more")
        else: out.append(f"{w[0]}: " + ", ".join(ts))
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- truth_table (medium, parsing)
@task("truth_table", r"""
Write a tool for boolean formulas. Each non-blank line of stdin is a command (lines numbered from 1):

- `table F` prints the truth table of formula F.
- `taut F` prints `tautology` if F is true for every assignment, else `not a tautology: ` followed by the first falsifying assignment.
- `sat F` prints `satisfiable: ` followed by the first satisfying assignment, or `unsatisfiable`.
- `equiv F ; G` prints `equivalent` if F and G agree on every assignment of their variables together, else `differ at ` followed by the first assignment where they differ.

Formulas: variables are a lowercase letter followed by lowercase letters or digits; `0` and `1` are constants; parentheses group. Operators, from lowest to highest precedence: `<->` (equivalence), `->` (implies, right-associative: `a -> b -> c` is `a -> (b -> c)`), `|` (or), `^` (exclusive or), `&` (and), `!` (not, prefix). `<->`, `|`, `^` and `&` are left-associative. Spaces between tokens are optional.

Assignments run over the formula's variables (for equiv, both formulas' variables) sorted as text, counting in binary from all 0s to all 1s with the first variable as the highest bit. An assignment is written `a=0 b=1`. With no variables, the only assignment is the empty one: write `not a tautology`, `satisfiable` and `differ` with nothing after them (no trailing space or colon).

A table prints a header line: each variable, then `|`, then F exactly as written after `table ` with spaces trimmed from both ends, all separated by single spaces. Then one line per assignment: each variable's value padded on the right to the width of its name, then `|`, then the result, separated by single spaces (with no variables a line is just `| 0`).

A formula that does not parse prints `line N: syntax error`; so does an equiv line without exactly one `;`, or a line starting with another word.
""", [
"""table a & b
table a -> b
taut a | !a
taut a -> b
sat a & !a
sat (a | b) & !a
equiv a -> b ; !a | b
equiv a ^ b ; (a | b) & !(a & b)
equiv a <-> b ; a ^ b
""",
"""table x1 ^ x2 ^ x3
table !(p->q)&r
taut ((p -> q) & (q -> r)) -> (p -> r)
taut p -> q -> p
table 1 & 0
taut 1 | 0
sat 0
equiv 1 ; a | !a
equiv a ; b
""",
"""table long & s
taut a -> -> b
sat (a & b
table a b
sat a & B
equiv a ; b ; c
equiv a | b
check a
taut a <-> b <-> a <-> b
table a | b & c
table (a | b) & c
""",
"""sat a1&a2&a3&!a4
taut (a & (a -> b)) -> b
taut a ^ a ^ a
equiv !(a & b) ; !a | !b
equiv !(a | b) ; !a | !b
table !!a
sat !1
table ab <-> b
""",
])
def _(s):
    out = []
    def lex(t):
        toks, i = [], 0
        while i < len(t):
            c = t[i]
            if c == " ": i += 1; continue
            if t.startswith("<->", i): toks.append("<->"); i += 3; continue
            if t.startswith("->", i): toks.append("->"); i += 2; continue
            if c in "|^&!()01": toks.append(c); i += 1; continue
            m = re.match(r"[a-z][a-z0-9]*", t[i:])
            if m: toks.append(m.group()); i += len(m.group()); continue
            raise SyntaxError
        return toks
    def parse(t):
        toks = lex(t); pos = [0]
        def peek(): return toks[pos[0]] if pos[0] < len(toks) else None
        def take(x=None):
            tk = peek()
            if tk is None or (x and tk != x): raise SyntaxError
            pos[0] += 1; return tk
        def left(sub, op):
            def f():
                a = sub()
                while peek() == op: take(); a = (op, a, sub())
                return a
            return f
        def un():
            tk = peek()
            if tk == "!": take(); return ("!", un())
            if tk == "(": take(); e = eq(); take(")"); return e
            if tk in ("0", "1"): take(); return ("c", tk == "1")
            if tk and re.fullmatch(r"[a-z][a-z0-9]*", tk): take(); return ("v", tk)
            raise SyntaxError
        andp = left(un, "&"); xorp = left(andp, "^"); orp = left(xorp, "|")
        def imp():
            a = orp()
            if peek() == "->": take(); return ("->", a, imp())
            return a
        eq = left(imp, "<->")
        e = eq()
        if pos[0] != len(toks): raise SyntaxError
        return e
    def ev(e, env):
        k = e[0]
        if k == "c": return e[1]
        if k == "v": return env[e[1]]
        if k == "!": return not ev(e[1], env)
        a, b = ev(e[1], env), ev(e[2], env)
        return {"&": a and b, "|": a or b, "^": a != b, "->": (not a) or b, "<->": a == b}[k]
    def vs(e, acc):
        if e[0] == "v": acc.add(e[1])
        elif e[0] != "c":
            for x in e[1:]: vs(x, acc)
        return acc
    def rows(vars_):
        for bits in itertools.product([0, 1], repeat=len(vars_)): yield dict(zip(vars_, bits))
    def show(env, vars_): return " ".join(f"{v}={env[v]}" for v in vars_)
    for n, line in enumerate(lines_of(s), 1):
        if not line.strip(): continue
        cmd, _, rest = line.strip().partition(" ")
        try:
            if cmd == "table":
                e = parse(rest); V = sorted(vs(e, set()))
                out.append(" ".join(V + ["|", rest.strip()]))
                for env in rows(V): out.append(" ".join([str(env[v]).ljust(len(v)) for v in V] + ["|", str(int(ev(e, env)))]))
            elif cmd in ("taut", "sat"):
                e = parse(rest); V = sorted(vs(e, set()))
                hit = next((env for env in rows(V) if ev(e, env) == (cmd == "sat")), None)
                if cmd == "taut": out.append("tautology" if hit is None else ("not a tautology: " + show(hit, V) if V else "not a tautology"))
                else: out.append("unsatisfiable" if hit is None else ("satisfiable: " + show(hit, V) if V else "satisfiable"))
            elif cmd == "equiv":
                if rest.count(";") != 1: raise SyntaxError
                fa, fb = rest.split(";"); a, b = parse(fa), parse(fb)
                V = sorted(vs(a, set()) | vs(b, set()))
                hit = next((env for env in rows(V) if ev(a, env) != ev(b, env)), None)
                out.append("equivalent" if hit is None else ("differ at " + show(hit, V) if V else "differ"))
            else: raise SyntaxError
        except SyntaxError:
            out.append(f"line {n}: syntax error")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- shop (large, 3 files)
@task("shop", r"""
Write a shop checkout with promotions. Lines of stdin are numbered from 1; blank lines are ignored. Money is decimal with 2 places and is always printed with exactly 2 decimals. SKUs, categories and IDs are words without spaces.

Setup lines (they may appear anywhere):
- `ITEM SKU CATEGORY PRICE`: PRICE is a positive amount with at most 2 decimals. A SKU already defined prints `line N: duplicate item`.
- `PROMO ID percent CATEGORY P`: P% off items of CATEGORY, P a whole number from 1 to 90.
- `PROMO ID bogo SKU`: buy one, get one free on SKU.
- `PROMO ID bundle SKU1 SKU2 PRICE`: one SKU1 plus one SKU2 together cost PRICE (SKU1 and SKU2 differ).
- `COUPON CODE AMOUNT MIN`: AMOUNT off a cart whose subtotal is at least MIN (both amounts with at most 2 decimals). Each code can be redeemed once in total.
A promo naming an undefined SKU prints `line N: unknown item`; a promo ID or coupon code already used prints `line N: duplicate promo` or `line N: duplicate coupon`. These lines are then ignored.

Cart lines:
- `CART ID` opens a cart. If a cart is already open, print `cart OLD abandoned` first.
- `ADD SKU QTY` and `REMOVE SKU QTY` (QTY a whole number from 1) change the open cart. ADD of an undefined SKU prints `line N: unknown item`; REMOVE of more than the cart holds prints `line N: not in cart`.
- `USE CODE` attaches a coupon to the open cart, replacing any earlier one. An unknown code prints `line N: unknown coupon`; a code already redeemed prints `line N: coupon used`.
- `CHECKOUT` prints the receipt (below) and closes the cart.
A cart line with no open cart prints `line N: no cart`. Any other line, or a line with the wrong shape or a bad value, prints `line N: bad line`.

Receipt: if the cart holds nothing, print `cart ID is empty`. Otherwise print `receipt ID`, then for each SKU in the cart, sorted by SKU, `  SKU QTY x PRICE = AMOUNT`. Then the promotions, each unit getting at most one, in this order:
1. Bundles, in the order the promos were defined: the number of pairs is the smaller of the units of SKU1 and SKU2 not yet used by a promo. If there is at least one pair and the bundle saves money, print `  bundle ID xPAIRS -SAVING` (SAVING = PAIRS times (price of SKU1 + price of SKU2 - PRICE)), and those units are used.
2. Bogo promos, in definition order: of the unused units of the SKU, every two units make one free unit. If at least one unit is free, print `  bogo ID SKU -SAVING` (free units times price); the paired units are used (an odd unit left over stays unused).
3. Percent promos, in definition order, but only the first percent promo defined for each category counts: for each SKU of that category in the cart (sorted by SKU) with unused units, print `  percent ID SKU -SAVING`, where SAVING is unused units times price times P / 100, rounded half up to 2 decimals.
Then `subtotal S` (amounts minus savings). If a coupon is attached: when S is at least MIN, print `  coupon CODE -D` with D the smaller of AMOUNT and S, and the code is redeemed; otherwise print `  coupon CODE needs MIN` and the code stays unredeemed. Then `total T`.

At the end of input, an open cart prints `cart ID abandoned`. Finally print `sales X in K carts`: the sum of the totals of the receipts printed, and how many there were.
""", [
"""ITEM apple fruit 0.50
ITEM pear fruit 0.75
ITEM bread bakery 2.20
ITEM jam pantry 3.10
PROMO p1 percent fruit 10
PROMO b1 bogo bread
PROMO k1 bundle bread jam 4.50
COUPON SAVE2 2 10
CART c1
ADD apple 4
ADD pear 3
ADD bread 3
ADD jam 1
CHECKOUT
CART c2
ADD jam 2
ADD bread 1
USE SAVE2
CHECKOUT
""",
"""ITEM tea drinks 4.99
ITEM mug kitchen 7.50
ITEM tea drinks 1.00
PROMO d1 percent drinks 15
PROMO d2 percent drinks 50
PROMO m1 bogo cup
PROMO d1 bogo tea
COUPON BIG 100 5
COUPON BIG 1 1
CART a
ADD tea 3
ADD mug 1
USE BIG
CHECKOUT
CART b
USE BIG
ADD tea 1
REMOVE tea 2
REMOVE tea 1
CHECKOUT
ADD tea 1
CART c
ADD milk 1
ADD tea 0
ADD tea two
FOO
CART d
ADD mug 2
""",
"""ITEM a x 1.00
ITEM b x 2.00
ITEM c y 0.99
PROMO z1 bundle a b 3.50
PROMO z2 bundle a b 2.50
PROMO z3 bogo a
PROMO z4 percent x 33
PROMO z5 percent y 5
COUPON TEN 10 20
COUPON ONE 1 0
CART r1
ADD a 5
ADD b 2
ADD c 7
USE TEN
CHECKOUT
CART r2
ADD c 1
USE TEN
USE ONE
CHECKOUT
CART r3
ADD c 1
USE ONE
CART r4
ADD a 1
ADD a 2
CHECKOUT
""",
"""CART early
COUPON HALF 5.5 5.5
ITEM pen office 1.25
PROMO o1 percent office 20
ITEM pad office 3.33
CART w
ADD pen 2
ADD pad 3
USE HALF
REMOVE pad 1
CHECKOUT
CART v
ADD pad 1
USE HALF
CHECKOUT
CART u
ADD pen 1
USE HALF
CHECKOUT
""",
], files=["main", "catalog", "pricing"])
def _(s):
    items, promos, coupons, used = {}, [], {}, set()
    pids = set()
    out = []; cart = None; sales = Decimal(0); nrec = 0
    M = r"\d+(\.\d{1,2})?"
    def money(x): return str(Decimal(x).quantize(Decimal("0.01")))
    def checkout(c):
        nonlocal sales, nrec
        lines = {k: q for k, q in c["items"].items() if q > 0}
        if not lines: out.append(f"cart {c['id']} is empty"); return
        out.append(f"receipt {c['id']}")
        sub = Decimal(0); avail = dict(lines)
        for k in sorted(lines):
            amt = items[k][1] * lines[k]; sub += amt
            out.append(f"  {k} {lines[k]} x {money(items[k][1])} = {money(amt)}")
        for p in promos:
            if p[1] == "bundle":
                _, _, s1, s2, price = p
                pairs = min(avail.get(s1, 0), avail.get(s2, 0))
                save = pairs * (items[s1][1] + items[s2][1] - price)
                if pairs > 0 and save > 0:
                    avail[s1] -= pairs; avail[s2] -= pairs; sub -= save
                    out.append(f"  bundle {p[0]} x{pairs} -{money(save)}")
        for p in promos:
            if p[1] == "bogo":
                k = p[2]; free = avail.get(k, 0) // 2
                if free > 0:
                    save = free * items[k][1]; avail[k] -= 2 * free; sub -= save
                    out.append(f"  bogo {p[0]} {k} -{money(save)}")
        seen = set()
        for p in promos:
            if p[1] == "percent" and p[2] not in seen:
                seen.add(p[2])
                for k in sorted(lines):
                    if items[k][0] == p[2] and avail.get(k, 0) > 0:
                        save = (avail[k] * items[k][1] * p[3] / Decimal(100)).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)
                        avail[k] = 0; sub -= save
                        out.append(f"  percent {p[0]} {k} -{money(save)}")
        out.append(f"subtotal {money(sub)}")
        tot = sub
        if c["coupon"]:
            code = c["coupon"]; amt, mn = coupons[code]
            if sub >= mn:
                d = min(amt, sub); tot = sub - d; used.add(code)
                out.append(f"  coupon {code} -{money(d)}")
            else:
                out.append(f"  coupon {code} needs {money(mn)}")
        out.append(f"total {money(tot)}")
        sales += tot; nrec += 1
    for n, line in enumerate(lines_of(s), 1):
        w = line.split()
        if not w: continue
        c0 = w[0]
        if c0 == "ITEM" and len(w) == 4 and re.fullmatch(M, w[3]) and Decimal(w[3]) > 0:
            if w[1] in items: out.append(f"line {n}: duplicate item"); continue
            items[w[1]] = (w[2], Decimal(w[3]))
        elif c0 == "PROMO" and len(w) >= 4 and (
                (w[2] == "percent" and len(w) == 5 and re.fullmatch(r"\d+", w[4]) and 1 <= int(w[4]) <= 90) or
                (w[2] == "bogo" and len(w) == 4) or
                (w[2] == "bundle" and len(w) == 6 and w[3] != w[4] and re.fullmatch(M, w[5]))):
            skus = w[3:4] if w[2] == "bogo" else (w[3:5] if w[2] == "bundle" else [])
            if any(k not in items for k in skus): out.append(f"line {n}: unknown item"); continue
            if w[1] in pids: out.append(f"line {n}: duplicate promo"); continue
            pids.add(w[1])
            if w[2] == "percent": promos.append((w[1], "percent", w[3], int(w[4])))
            elif w[2] == "bogo": promos.append((w[1], "bogo", w[3]))
            else: promos.append((w[1], "bundle", w[3], w[4], Decimal(w[5])))
        elif c0 == "COUPON" and len(w) == 4 and re.fullmatch(M, w[2]) and re.fullmatch(M, w[3]):
            if w[1] in coupons: out.append(f"line {n}: duplicate coupon"); continue
            coupons[w[1]] = (Decimal(w[2]), Decimal(w[3]))
        elif c0 == "CART" and len(w) == 2:
            if cart: out.append(f"cart {cart['id']} abandoned")
            cart = {"id": w[1], "items": {}, "coupon": None}
        elif (c0 in ("ADD", "REMOVE") and len(w) == 3 and re.fullmatch(r"\d+", w[2]) and int(w[2]) >= 1) or (c0 == "USE" and len(w) == 2) or (w == ["CHECKOUT"]):
            if not cart: out.append(f"line {n}: no cart"); continue
            if c0 == "ADD":
                if w[1] not in items: out.append(f"line {n}: unknown item"); continue
                cart["items"][w[1]] = cart["items"].get(w[1], 0) + int(w[2])
            elif c0 == "REMOVE":
                if cart["items"].get(w[1], 0) < int(w[2]): out.append(f"line {n}: not in cart"); continue
                cart["items"][w[1]] -= int(w[2])
            elif c0 == "USE":
                if w[1] not in coupons: out.append(f"line {n}: unknown coupon"); continue
                if w[1] in used: out.append(f"line {n}: coupon used"); continue
                cart["coupon"] = w[1]
            else:
                checkout(cart); cart = None
        else:
            out.append(f"line {n}: bad line")
    if cart: out.append(f"cart {cart['id']} abandoned")
    out.append(f"sales {money(sales)} in {nrec} carts")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- chat_rooms (large, one file)
@task("chat_rooms", r"""
Simulate chat rooms. Lines of stdin are numbered from 1; blank lines are ignored. User names are letters and digits; room names are `#` followed by letters and digits. A line with the wrong number of words, a bad name or an unknown command prints `line N: bad line`.

- `JOIN U R`: U joins room R. If R does not exist it is created and U is its owner. If U is already in R print `line N: already in room`.
- `LEAVE U R`: U leaves R. If the owner leaves, the member who joined earliest among those left becomes the owner. A room left empty is deleted, with its messages and mutes. Leaving also forgets U's mute in R.
- `SAY U R TEXT...`: U posts TEXT (the rest of the line after R and the spaces that follow it, kept as written; at least one word) in R. A muted user prints `line N: muted`. Every distinct word of TEXT that is exactly `@` followed by the name of another member of R gives that member one mention in R.
- `MUTE O U R` and `UNMUTE O U R`: the owner O mutes or unmutes member U. If O is not R's owner print `line N: not owner`; if U is not in R print `line N: not in room`; if U is O print `line N: not allowed`.
- `READ U R`: shows U the messages posted in R by other users since U joined R or last read it, whichever is later. With none, print `R: nothing new for U`. Otherwise print `R: K new for U`, then `  (E earlier)` if there are more than 3 (E = K - 3), then the last 3 (or fewer) as `  SENDER: TEXT`. Reading also clears U's mentions in R.
- `WHO R`: prints `R: ` and the members sorted by name, separated by `, `, with `*` after the owner and `(muted)` after a muted member (`#a: amy*, bob(muted), cal`).
- `INBOX U`: prints `U: ` followed by, for each room U is in sorted by name, `R new=K at=M` (K messages READ would show now, M mentions), separated by `; `. If U is in no room, print `U: no rooms`.
- `STATS`: prints `rooms R, users U, messages M`: rooms that exist, distinct users in at least one room, and all messages ever posted.

For LEAVE, SAY, MUTE, UNMUTE, READ and WHO, a room that does not exist prints `line N: no such room`; for LEAVE, SAY and READ, a user not in the room prints `line N: not in room`. Check the room first, then the rest in the order given above.
""", [
"""JOIN amy #gen
JOIN bob #gen
JOIN cal #gen
SAY amy #gen hello all
SAY bob #gen hi @amy and @cal @amy
SAY cal #gen @bob   welcome
READ amy #gen
READ amy #gen
INBOX bob
WHO #gen
MUTE amy bob #gen
SAY bob #gen am I muted
WHO #gen
UNMUTE amy bob #gen
SAY bob #gen back
READ cal #gen
STATS
""",
"""JOIN ann #a
JOIN ben #a
JOIN ann #b
JOIN ann #a
LEAVE ann #a
WHO #a
MUTE ann ben #a
MUTE ben ann #a
MUTE ben ben #a
SAY ben #a one
SAY ben #a two
SAY ben #a three
SAY ben #a four
SAY ben #a five
JOIN ann #a
SAY ben #a six
READ ann #a
LEAVE ben #a
LEAVE ann #a
WHO #a
SAY ann #a ghost
READ ben #b
INBOX ann
INBOX ben
STATS
""",
"""JOIN x1 #r1
JOIN x2 #r1
JOIN x3 #r1
SAY x2 #r1 @x1 @x1 @x3 @x2 @nobody
SAY x3 #r1 @x1, hi
INBOX x1
INBOX x3
MUTE x1 x3 #r1
LEAVE x3 #r1
JOIN x3 #r1
SAY x3 #r1 free again
LEAVE x1 #r1
WHO #r1
MUTE x2 x3 #r1
READ x2 #r1
READ x2 #r1
SAY x1 #r1 not here
JOIN bad name #r1
JOIN x1 room
JOIN x1
SAY x2 #r1
SHOUT x2 #r1 hey
STATS
""",
"""JOIN zed #z
SAY zed #z talking to myself
READ zed #z
JOIN amy #z
SAY zed #z m1
SAY zed #z m2
SAY amy #z reply
SAY zed #z m3 @amy
SAY zed #z m4
READ amy #z
INBOX amy
INBOX zed
READ zed #z
WHO #z
READ bob #z
UNMUTE amy zed #z
MUTE zed amy #q
INBOX nobody
""",
])
def _(s):
    rooms = {}   # name -> {"members": [names in join order], "owner", "muted": set, "msgs": [(seq, sender, text)], "seen": {u: seq}, "ment": {u: n}}
    out = []; total = 0; seq = 0
    U, R = r"[A-Za-z0-9]+", r"#[A-Za-z0-9]+"
    def unread(r, u): return [m for m in r["msgs"] if m[0] > r["seen"][u] and m[1] != u]
    for n, line in enumerate(lines_of(s), 1):
        w = line.split()
        if not w: continue
        c = w[0]
        shapes = {"JOIN": 3, "LEAVE": 3, "READ": 3, "MUTE": 4, "UNMUTE": 4, "WHO": 2, "INBOX": 2, "STATS": 1}
        ok = False
        if c in shapes and len(w) == shapes[c]:
            if c in ("JOIN", "LEAVE", "READ"): ok = re.fullmatch(U, w[1]) and re.fullmatch(R, w[2])
            elif c in ("MUTE", "UNMUTE"): ok = re.fullmatch(U, w[1]) and re.fullmatch(U, w[2]) and re.fullmatch(R, w[3])
            elif c == "WHO": ok = re.fullmatch(R, w[1])
            elif c == "INBOX": ok = re.fullmatch(U, w[1])
            else: ok = True
        elif c == "SAY" and len(w) >= 4: ok = re.fullmatch(U, w[1]) and re.fullmatch(R, w[2])
        if not ok: out.append(f"line {n}: bad line"); continue
        if c == "JOIN":
            u, rn = w[1], w[2]
            r = rooms.setdefault(rn, {"members": [], "owner": u, "muted": set(), "msgs": [], "seen": {}, "ment": {}})
            if u in r["members"]: out.append(f"line {n}: already in room"); continue
            r["members"].append(u); r["seen"][u] = seq; r["ment"][u] = 0
        elif c == "STATS":
            users = {u for r in rooms.values() for u in r["members"]}
            out.append(f"rooms {len(rooms)}, users {len(users)}, messages {total}")
        elif c == "INBOX":
            u = w[1]; mine = sorted(rn for rn, r in rooms.items() if u in r["members"])
            out.append(f"{u}: " + ("; ".join(f"{rn} new={len(unread(rooms[rn], u))} at={rooms[rn]['ment'][u]}" for rn in mine) if mine else "no rooms"))
        else:
            rn = w[3] if c in ("MUTE", "UNMUTE") else (w[1] if c == "WHO" else w[2])
            if rn not in rooms: out.append(f"line {n}: no such room"); continue
            r = rooms[rn]
            if c == "WHO":
                out.append(f"{rn}: " + ", ".join(m + ("*" if m == r["owner"] else "") + ("(muted)" if m in r["muted"] else "") for m in sorted(r["members"])))
                continue
            if c in ("MUTE", "UNMUTE"):
                o, u = w[1], w[2]
                if r["owner"] != o: out.append(f"line {n}: not owner"); continue
                if u not in r["members"]: out.append(f"line {n}: not in room"); continue
                if u == o: out.append(f"line {n}: not allowed"); continue
                if c == "MUTE": r["muted"].add(u)
                else: r["muted"].discard(u)
                continue
            u = w[1]
            if u not in r["members"]: out.append(f"line {n}: not in room"); continue
            if c == "LEAVE":
                r["members"].remove(u); r["muted"].discard(u); del r["seen"][u]; del r["ment"][u]
                if not r["members"]: del rooms[rn]
                elif r["owner"] == u: r["owner"] = r["members"][0]
            elif c == "SAY":
                if u in r["muted"]: out.append(f"line {n}: muted"); continue
                text = line.split(None, 3)[3]
                seq += 1; total += 1; r["msgs"].append((seq, u, text))
                for word in set(text.split()):
                    if word.startswith("@") and word[1:] in r["members"] and word[1:] != u: r["ment"][word[1:]] += 1
            elif c == "READ":
                new = unread(r, u); r["seen"][u] = seq; r["ment"][u] = 0
                if not new: out.append(f"{rn}: nothing new for {u}"); continue
                out.append(f"{rn}: {len(new)} new for {u}")
                if len(new) > 3: out.append(f"  ({len(new) - 3} earlier)")
                for _, snd, text in new[-3:]: out.append(f"  {snd}: {text}")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- scheduler (large, 3 files)
@task("scheduler", r"""
Plan jobs on a pool of workers. Lines of stdin are numbered from 1; blank lines are ignored. Job IDs are letters, digits, `_` and `-`.

- `WORKERS K` sets the number of workers (K at least 1) for later runs. Before any WORKERS line there is 1 worker.
- `JOB ID DURATION [after=ID,ID,...] [prio=P]` defines a job. DURATION is a whole number from 1; P is a whole number that may be negative (default 0). The two options may come in either order, each at most once; `after=` lists one or more IDs this job must wait for (they may be defined later). A job ID already defined prints `line N: duplicate job` and the line is ignored.
- `RUN` plans all jobs defined so far (it may appear several times; later runs include later jobs).
Anything else, or a line with a bad value, prints `line N: bad line`.

A RUN first checks the jobs:
1. If any job waits for an ID that is not defined, print `job X needs unknown Y` for each such pair, by job ID (sorted as text) and then in the order Y is listed, and skip this run.
2. Otherwise, if some jobs wait on each other in a cycle (including a job waiting for itself), print `cycle among: A,B,...` with every job that is on a cycle, sorted, and skip this run.

Otherwise print `plan with K workers`, then simulate. Workers are numbered 1 to K. Time starts at 0. At each moment, first every job ending then finishes; then, while a worker is free and a job is ready (not started, and every job it waits for has finished), the lowest-numbered free worker starts the ready job with the highest priority, then the longest duration, then the smallest ID as text, printing `  T start ID on wX`. Time then moves on to the next moment a job ends. When all jobs are done print `makespan M` (the time the last job ends; 0 if there are no jobs).

Then print `critical path A > B > C (L)`: the chain of jobs with the largest total duration L, where each job in the chain waits for the one before it. On a tie, take the chain whose list of IDs comes first when compared ID by ID as text (a chain that is a prefix of another comes first). With no jobs print `critical path none`.
""", [
"""WORKERS 2
JOB design 3
JOB build 5 after=design
JOB test 2 after=build
JOB docs 2 after=design prio=1
JOB ship 1 after=test,docs
RUN
""",
"""JOB a 2
JOB b 2
JOB c 4 prio=-1
RUN
WORKERS 3
JOB d 1 after=a,b,c
JOB e 3 prio=2 after=a
RUN
JOB a 9
JOB f 1 after=g
JOB h 1 after=zz,f,yy
RUN
JOB g 2
JOB zz 1
JOB yy 1
RUN
""",
"""JOB x 1 after=y
JOB y 1 after=z
JOB z 1 after=x
JOB w 2 after=w
JOB v 1 after=x
JOB u 1
RUN
WORKERS 0
WORKERS two
JOB q 0
JOB r 1 prio=high
JOB s 1 after=
JOB t 1 after=u after=u
JOB t2 1 color=red
PLAN
""",
"""RUN
WORKERS 4
JOB m1 4
JOB m2 4
JOB m3 4
JOB m4 4
JOB m5 1 prio=5
JOB k 2 after=m1,m2
JOB j 2 after=m3,m4
JOB end 1 after=k,j,m5
RUN
WORKERS 1
RUN
""",
], files=["main", "jobs", "plan"])
def _(s):
    jobs = {}  # id -> (dur, deps, prio)
    K = 1; out = []
    ID = r"[A-Za-z0-9_-]+"
    for n, line in enumerate(lines_of(s), 1):
        w = line.split()
        if not w: continue
        if w[0] == "WORKERS" and len(w) == 2 and re.fullmatch(r"\d+", w[1]) and int(w[1]) >= 1:
            K = int(w[1]); continue
        if w[0] == "JOB" and 3 <= len(w) <= 5 and re.fullmatch(ID, w[1]) and re.fullmatch(r"\d+", w[2]) and int(w[2]) >= 1:
            deps, prio, good, seenopt = [], 0, True, set()
            for o in w[3:]:
                m1 = re.fullmatch(r"after=(" + ID + r"(?:," + ID + r")*)", o)
                m2 = re.fullmatch(r"prio=(-?\d+)", o)
                if m1 and "after" not in seenopt: deps = m1.group(1).split(","); seenopt.add("after")
                elif m2 and "prio" not in seenopt: prio = int(m2.group(1)); seenopt.add("prio")
                else: good = False
            if good:
                if w[1] in jobs: out.append(f"line {n}: duplicate job"); continue
                jobs[w[1]] = (int(w[2]), deps, prio); continue
        if w == ["RUN"]:
            unk = [(j, d) for j in sorted(jobs) for d in jobs[j][1] if d not in jobs]
            if unk:
                for j, d in unk: out.append(f"job {j} needs unknown {d}")
                continue
            # jobs on a cycle: j reaches itself through deps
            def reach(a):
                seen, st = set(), list(jobs[a][1])
                while st:
                    x = st.pop()
                    if x not in seen: seen.add(x); st += jobs[x][1]
                return seen
            cyc = sorted(j for j in jobs if j in reach(j))
            if cyc: out.append("cycle among: " + ",".join(cyc)); continue
            out.append(f"plan with {K} workers")
            t, done, started, running = 0, set(), set(), {}   # worker -> (end, id)
            while len(done) < len(jobs):
                for x in list(running):
                    if running[x][0] == t: done.add(running[x][1]); del running[x]
                ready = sorted((j for j in jobs if j not in started and all(d in done for d in jobs[j][1])), key=lambda j: (-jobs[j][2], -jobs[j][0], j))
                for x in range(1, K + 1):
                    if x not in running and ready:
                        j = ready.pop(0); started.add(j); running[x] = (t + jobs[j][0], j)
                        out.append(f"  {t} start {j} on w{x}")
                if running: t = min(e for e, _ in running.values())
            out.append(f"makespan {t}")
            if not jobs: out.append("critical path none"); continue
            memo = {}
            def best(j):
                if j not in memo:
                    nxt = [best(k) for k in jobs if j in jobs[k][1]]
                    b = min(nxt, key=lambda c: (-c[0], c[1])) if nxt else (0, [])
                    memo[j] = (jobs[j][0] + b[0], [j] + b[1])
                return memo[j]
            L, path = min((best(j) for j in jobs), key=lambda c: (-c[0], c[1]))
            out.append(f"critical path {' > '.join(path)} ({L})")
            continue
        out.append(f"line {n}: bad line")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- json_query (large, 3 files)
DOC1 = """{
  "store": {
    "name": "Corner Books",
    "open": true,
    "books": [
      {"title": "Dune", "author": "Herbert", "price": 9.5, "tags": ["sf", "classic"], "stock": 3},
      {"title": "Emma", "author": "Austen", "price": 4, "tags": ["classic"], "stock": 0},
      {"title": "Neuromancer", "author": "Gibson", "price": 12.25, "tags": ["sf"], "stock": 7},
      {"title": "Odd", "author": null, "price": "n/a", "tags": [], "stock": 1}
    ],
    "staff": {"mgr": "Kim", "clerk": "Lou"}
  },
  "visits": [12, 7, 30, 5]
}"""
@task("json_query", r"""
Answer queries about a JSON document. Stdin holds the document (over one or more lines) up to a line that is exactly `%%`; each non-blank line after it is a query. Queries are numbered from 1.

A query is a chain of steps, optionally followed by `|` and a function name. Spaces between tokens are allowed anywhere (but not inside a key, number or string). A query that is only `.` means the whole document. Steps:
- `.key` where key is a letter or `_` followed by letters, digits or `_`; or `["any key"]` with a JSON string. Picks that member of an object.
- `[N]` with N a whole number, maybe negative (counting from the end, `-1` is the last). Picks that element of an array.
- `[*]` picks every element of an array, or every member value of an object in document order.
- `[?key OP VALUE]` keeps the elements of an array that are objects having member `key` whose value compares true with VALUE. OP is `==`, `!=`, `<`, `<=`, `>` or `>=`; VALUE is a JSON number, string, `true`, `false` or `null`. `==` and `!=` compare as JSON (numbers by value; `true` is not `1`); the other four are true only when both sides are numbers (not booleans) or both are strings (compared as text).

The query starts with a list holding the document, and each step replaces every value in the list with what it picks from it, in order. A value a step cannot pick from (a missing key, an index out of range, or the wrong type) gives nothing.

Functions, applied after all steps: `count` gives one value, how many values there are; `sum` gives one value, the sum of the values that are numbers (not booleans), an integer if they all are, else a float (0 if none); `first` keeps only the first value, if any; `keys` replaces each object with the sorted list of its keys and drops other values; `length` replaces each string, array or object with its length and drops other values.

Print each resulting value on its own line as Python's `json.dumps(value)` prints it (default settings; objects keep their key order). If there are no values, print `(none)`. A query that does not fit this grammar prints `query K: syntax error`.
""", [
DOC1 + """
%%
.store.name
.store.books[0].title
.store.books[*].title
.store.books[?price < 10].title
.store.books[-1]
.visits[*] | sum
.visits | count
.store.books[*].tags | length
.store.staff | keys
.store.staff[*]
.store.books[?author == null].title
.store.books[?stock > 0] | count
.store.books[9].title
.
""",
DOC1 + """
%%
.store.books[?tags != []].title
.store.books[*].price | sum
.store["name"]
.store [ "books" ] [ 1 ] . author
.store.books[?title >= "E"].title | first
.store.open
.store.books[?price > "a"].title
.visits[1:2]
.store.books[
store
.store.books[?price ~ 3]
| count
.missing | count
.store.books[*].tags[0]
""",
"""[{"k": 1, "v": true}, {"k": 2, "v": 1}, {"k": 2.0, "v": false}, {"v": null}, 5, "x"]
%%
.[?k == 2].v
.[?v == 1] | count
.[?v != 1] | count
.[*] | length
.[*].k | sum
.[4]
.[-6].k
.[-7]
.[*] | keys
.[?k >= 1.5]
.[0]["k"]
""",
"""{"a b": {"c-d": [1, [2, 3], {"e": "f"}]}, "n": -0.5, "s": "text", "z": {}}
%%
.["a b"]["c-d"][1][1]
.["a b"]["c-d"][*] | length
.n
.s | length
.z | keys
.z[*]
.z | count
. | keys
.["a b"].c-d
.["a b"]["c-d"][2].e
.[*] | first
.["a b"]["c-d"][0] | sum
""",
], files=["main", "lexer", "query"])
def _(s):
    ls = lines_of(s)
    k = ls.index("%%")
    doc = json.loads("\n".join(ls[:k]))
    out = []
    class SynErr(Exception): pass
    def lex(q):
        toks, i = [], 0
        while i < len(q):
            c = q[i]
            if c == " ": i += 1; continue
            if c in ".[]*?|": toks.append(("p", c)); i += 1; continue
            if q.startswith(("==", "!=", "<=", ">="), i): toks.append(("op", q[i:i+2])); i += 2; continue
            if c in "<>": toks.append(("op", c)); i += 1; continue
            if c == '"':
                m = re.match(r'"(?:[^"\\]|\\.)*"', q[i:])
                if not m: raise SynErr
                try: toks.append(("str", json.loads(m.group())))
                except ValueError: raise SynErr
                i += len(m.group()); continue
            m = re.match(r"-?\d+(\.\d+)?([eE][+-]?\d+)?", q[i:])
            if m: toks.append(("num", json.loads(m.group()))); i += len(m.group()); continue
            m = re.match(r"[A-Za-z_][A-Za-z0-9_]*", q[i:])
            if m: toks.append(("id", m.group())); i += len(m.group()); continue
            raise SynErr
        return toks
    def jeq(a, b):
        if isinstance(a, bool) or isinstance(b, bool): return type(a) is type(b) and a == b
        if isinstance(a, (int, float)) and isinstance(b, (int, float)): return a == b
        if isinstance(a, dict) and isinstance(b, dict): return a.keys() == b.keys() and all(jeq(a[x], b[x]) for x in a)
        if isinstance(a, list) and isinstance(b, list): return len(a) == len(b) and all(jeq(x, y) for x, y in zip(a, b))
        return type(a) is type(b) and a == b
    def isnum(x): return isinstance(x, (int, float)) and not isinstance(x, bool)
    def cmp(a, op, b):
        if op == "==": return jeq(a, b)
        if op == "!=": return not jeq(a, b)
        if not ((isnum(a) and isnum(b)) or (isinstance(a, str) and isinstance(b, str))): return False
        return {"<": a < b, "<=": a <= b, ">": a > b, ">=": a >= b}[op]
    def parse(toks):
        pos = [0]
        def peek(j=0): return toks[pos[0] + j] if pos[0] + j < len(toks) else (None, None)
        def take(kind=None, val=None):
            t = peek()
            if t[0] is None or (kind and t[0] != kind) or (val and t[1] != val): raise SynErr
            pos[0] += 1; return t
        steps = []
        if peek() == ("p", ".") and peek(1)[0] in (None,) or (peek() == ("p", ".") and peek(1) == ("p", "|")):
            take(); steps = []
        else:
            if peek()[0] is None: raise SynErr
            while peek()[0] is not None and peek() != ("p", "|"):
                t = take()
                if t == ("p", "."):
                    if peek()[0] == "id": steps.append(("key", take()[1]))
                    elif peek() == ("p", "["): steps.append(("noop",))
                    else: raise SynErr
                elif t == ("p", "["):
                    nt = peek()
                    if nt[0] == "str": steps.append(("key", take()[1]))
                    elif nt[0] == "num" and isinstance(nt[1], int): steps.append(("idx", take()[1]))
                    elif nt == ("p", "*"): take(); steps.append(("all",))
                    elif nt == ("p", "?"):
                        take(); key = take("id")[1]; op = take("op")[1]; v = take()
                        if v[0] in ("num", "str"): val = v[1]
                        elif v[0] == "id" and v[1] in ("true", "false", "null"): val = {"true": True, "false": False, "null": None}[v[1]]
                        else: raise SynErr
                        steps.append(("filt", key, op, val))
                    else: raise SynErr
                    take("p", "]")
                else: raise SynErr
        fn = None
        if peek() == ("p", "|"):
            take(); fn = take("id")[1]
            if fn not in ("count", "sum", "first", "keys", "length"): raise SynErr
        if peek()[0] is not None: raise SynErr
        return [st for st in steps if st != ("noop",)], fn
    def check_start(toks):
        # every query starts with '.'
        if not toks or toks[0] != ("p", "."): raise SynErr
    for qn, q in enumerate([l for l in ls[k + 1:] if l.strip()], 1):
        try:
            toks = lex(q); check_start(toks)
            steps, fn = parse(toks)
        except SynErr:
            out.append(f"query {qn}: syntax error"); continue
        vals = [doc]
        for st in steps:
            nv = []
            for v in vals:
                if st[0] == "key":
                    if isinstance(v, dict) and st[1] in v: nv.append(v[st[1]])
                elif st[0] == "idx":
                    if isinstance(v, list) and -len(v) <= st[1] < len(v): nv.append(v[st[1]])
                elif st[0] == "all":
                    if isinstance(v, list): nv += v
                    elif isinstance(v, dict): nv += list(v.values())
                else:
                    if isinstance(v, list):
                        nv += [e for e in v if isinstance(e, dict) and st[1] in e and cmp(e[st[1]], st[2], st[3])]
            vals = nv
        if fn == "count": vals = [len(vals)]
        elif fn == "sum":
            ns = [v for v in vals if isnum(v)]
            vals = [sum(ns) if all(isinstance(v, int) for v in ns) else float(sum(ns))]
        elif fn == "first": vals = vals[:1]
        elif fn == "keys": vals = [sorted(v) for v in vals if isinstance(v, dict)]
        elif fn == "length": vals = [len(v) for v in vals if isinstance(v, (str, list, dict))]
        out += [json.dumps(v) for v in vals] if vals else ["(none)"]
    return "\n".join(out) + "\n"

ROLES = {
    "shop": "`catalog` reads and checks the ITEM, PROMO and COUPON lines and holds them; `pricing` works out a receipt from a cart and the catalog; `main` reads stdin, keeps the open cart and the sales total, and prints.",
    "scheduler": "`jobs` reads and checks the WORKERS and JOB lines and holds the job table; `plan` checks for unknown jobs and cycles, simulates the workers and finds the critical path; `main` reads stdin and prints.",
    "json_query": "`lexer` turns a query into tokens; `query` parses the tokens into steps and runs them on the document; `main` reads stdin, splits off the document and prints the results.",
}

if __name__ == "__main__":
    only = sys.argv[1:]
    for name, (prompt, inputs, fn) in T.items():
        if only and name not in only: continue
        d = f"tasks/{name}"; os.makedirs(f"{d}/tests", exist_ok=True)
        if name in FILES:
            fs = FILES[name]
            prompt += "\n" + MULTI.format(n=len(fs), files=", ".join(f"`{f}`" for f in fs), roles=ROLES[name]).rstrip()
            open(f"{d}/files", "w").write(" ".join(fs) + "\n")
        open(f"{d}/prompt.md", "w").write(prompt + "\n")
        for i, inp in enumerate(inputs):
            open(f"{d}/tests/{i}.in", "w").write(inp)
            open(f"{d}/tests/{i}.out", "w").write(fn(inp))
    print(len(T), "tasks")
