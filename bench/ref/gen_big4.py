# Held-out XL tasks, second set (written 2026-10-31, after Brevi v0.18; never used to tune the language).
# Same size as gen_big3.py (150 to 400 lines); two of them must be split over several files.
# Writes tasks/<name>/{prompt.md,tests/N.in,N.out} and, for multi-file tasks, tasks/<name>/files.
import os, re
from decimal import Decimal, ROUND_HALF_UP
import datetime
T = {}
FILES = {}
def task(name, prompt, inputs, files=None):
    def deco(fn):
        T[name] = (prompt.strip(), inputs, fn)
        if files: FILES[name] = files
        return fn
    return deco

def d2(x): return str(Decimal(x).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))

MULTI = """
Write the program as {n} files in the folder you are given: {files}. `main` is the entry point; each file imports what it needs from the others. {roles}
"""

# ---------------------------------------------------------------- grep_lite
@task("grep_lite", r"""
Write a small regular-expression search. The first line of stdin is a pattern; every later line is a text line to search.

Pattern syntax:
- a plain character matches itself; `\` followed by any character matches that character literally (`\.`, `\*`, `\\`).
- `.` matches any one character.
- `[abc]` matches one of the listed characters, `[a-z]` a range (ranges and single characters can be mixed: `[a-cx_0-9]`), and `[^...]` any character not in the set. Inside brackets every character is literal except `^` right after `[`, `-` between two characters, and `]`, which closes the set. A `-` first or last in the set is literal.
- `*` (0 or more), `+` (1 or more), `?` (0 or 1), `{n}` (exactly n) and `{m,n}` (m to n) repeat the item just before them. n and m are whole numbers.
- `^` as the very first character anchors the match to the start of the line, and `$` as the very last character anchors it to the end. Anywhere else they are plain characters.

For each text line (numbered from 1, blank lines included), find the match that starts leftmost; among the matches starting there, take the longest. If the line matches, print `N: S-E [TEXT]`: the line number, the start and end position (0-based, end exclusive) and the matched text. An empty match counts as a match.

After the last line print `K of T lines match`.

If the pattern is invalid, print only `bad pattern` and nothing else. A pattern is invalid if it has a `[` with no closing `]`, an empty set `[]` or `[^]`, a range whose start is after its end (`[z-a]`), a repeat with nothing before it (at the start, or right after `^`), a repeat right after another repeat (`a**`, `a+?`), a `{` that is not `{n}` or `{m,n}`, `{m,n}` with m greater than n, or a `\` at the very end.
""", [
r"""colou?r
The color is red.
Colour me surprised: colour!
no match here
colouur
""",
r"""^[A-Z][a-z]+ [0-9]{2,3}\.[0-9]*$
Alice 42.5
Bob 7.0
Carol 123.
dave 99.9
Eve 1000.1
Frank 55.

Zed 10.25
""",
r"""[^aeiou ]+x?
aeiou
the quick fox
x
a{b}
""",
r"""a**
abc
""",
r"""[a-c-]{2}\-.*
--ab-c
zz-a
b--
""",
]
)
def _(s):
    lines = s.split("\n")
    if lines and lines[-1] == "": lines.pop()
    pat = lines[0]; texts = lines[1:]
    BAD = "bad pattern\n"
    i = 0; items = []; anch_s = anch_e = False
    if pat.startswith("^"): anch_s = True; i = 1
    end = len(pat)
    if end > i and pat.endswith("$") and not (pat.endswith("\\$") and not pat.endswith("\\\\$")):
        anch_e = True; end -= 1
    last_rep = False
    while i < end:
        c = pat[i]
        if c in "*+?{":
            if not items or last_rep: return BAD
            if c == "*": lo, hi = 0, None; i += 1
            elif c == "+": lo, hi = 1, None; i += 1
            elif c == "?": lo, hi = 0, 1; i += 1
            else:
                m = re.match(r"\{(\d+)(?:,(\d+))?\}", pat[i:end])
                if not m: return BAD
                lo = int(m[1]); hi = int(m[2]) if m[2] is not None else lo
                if lo > hi: return BAD
                i += len(m[0])
            items[-1] = (items[-1][0], lo, hi); last_rep = True
            continue
        last_rep = False
        if c == "\\":
            if i + 1 >= len(pat): return BAD
            ch = pat[i + 1]; items.append(((lambda ch: lambda x: x == ch)(ch), 1, 1)); i += 2
        elif c == ".":
            items.append((lambda x: True, 1, 1)); i += 1
        elif c == "[":
            j = i + 1; neg = False
            if j < end and pat[j] == "^": neg = True; j += 1
            k = pat.find("]", j, end)
            if k < 0: return BAD
            body = pat[j:k]
            if body == "": return BAD
            singles = set(); ranges = []
            p = 0
            while p < len(body):
                if p + 2 < len(body) and body[p + 1] == "-":
                    a, b = body[p], body[p + 2]
                    if a > b: return BAD
                    ranges.append((a, b)); p += 3
                else:
                    singles.add(body[p]); p += 1
            f = (lambda singles, ranges, neg: lambda x: (x in singles or any(a <= x <= b for a, b in ranges)) != neg)(singles, ranges, neg)
            items.append((f, 1, 1)); i = k + 1
        else:
            items.append(((lambda ch: lambda x: x == ch)(c), 1, 1)); i += 1
    def ends(t, k, pos, memo):
        key = (k, pos)
        if key in memo: return memo[key]
        if k == len(items):
            r = {pos} if (not anch_e or pos == len(t)) else set()
            memo[key] = r; return r
        f, lo, hi = items[k]; r = set(); p = pos; cnt = 0
        while True:
            if cnt >= lo: r |= ends(t, k + 1, p, memo)
            if hi is not None and cnt == hi: break
            if p < len(t) and f(t[p]): p += 1; cnt += 1
            else: break
        memo[key] = r; return r
    out = []; K = 0
    for n, t in enumerate(texts, 1):
        starts = [0] if anch_s else range(len(t) + 1)
        for st in starts:
            e = ends(t, 0, st, {})
            if e:
                E = max(e); out.append(f"{n}: {st}-{E} [{t[st:E]}]"); K += 1; break
    out.append(f"{K} of {len(texts)} lines match")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- payroll
@task("payroll", """
Run a small monthly payroll. Read commands from stdin, one per line (ignore blank lines):

- `EMP ID NAME hourly RATE` or `EMP ID NAME salary ANNUAL`: add an employee. ID and NAME are words; RATE (per hour) and ANNUAL (per year) are positive decimals with at most 2 decimals.
- `HOURS ID DATE H`: hours worked by an hourly employee on DATE (`YYYY-MM-DD`, a real date). H is a decimal with at most 2 decimals, more than 0. Several HOURS lines for the same day add up.
- `BONUS ID YYYY-MM AMOUNT`: a one-off bonus paid in that month, for any employee. AMOUNT is a positive decimal with at most 2 decimals.
- `RUN YYYY-MM`: print the payroll for that month.

Pay for a month:
- salaried: ANNUAL / 12.
- hourly, per day worked in that month: on Saturday and Sunday every hour is paid at 2 times RATE; on other days the first 8 hours of the day are paid at RATE and the hours above 8 at 1.5 times RATE.
- gross = that pay plus the month's bonuses, rounded half up to cents.
- tax on gross: 0% on the first 1000, 20% on the part from 1000 to 4000, 40% on the part above 4000, rounded half up to cents. net = gross - tax.

`RUN` prints `payroll YYYY-MM`, then one line per employee in ID order (string order): two spaces, `ID NAME gross G tax T net N`, then `  total gross G tax T net N` summing the lines above. All money with 2 decimals. Employees with nothing to pay are listed with 0.00. With no employees print just the header and the total line.

Errors print one line and change nothing: `line N: bad line` for a line not exactly in one of the forms above (N counts every input line, blank ones included), `line N: duplicate ID`, `line N: unknown ID`, `line N: not hourly` for HOURS on a salaried employee, and `line N: too many hours` when the day's total for that employee would go above 24.
""", [
"""EMP e2 Bob salary 54000
EMP e1 Ann hourly 20
HOURS e1 2026-03-02 10
HOURS e1 2026-03-03 8
HOURS e1 2026-03-07 5.5
HOURS e1 2026-03-07 2
HOURS e1 2026-04-01 9
BONUS e2 2026-03 1500.50
RUN 2026-03
RUN 2026-04
""",
"""EMP a1 Zoe hourly 31.25
EMP a1 Zed salary 1
HOURS a1 2026-02-30 3
HOURS a1 2026-02-28 20
HOURS a1 2026-02-28 5
HOURS b9 2026-02-02 1
HOURS a1 2026-02-02 0
EMP s1 Sam salary 9000.99
HOURS s1 2026-02-02 4
BONUS s1 2026-2 10
BONUS s1 2026-02 10
hours a1 2026-02-03 1

RUN 2026-02
RUN 2026-13
""",
"""RUN 2026-01
EMP x Max hourly 12.345
EMP x Max hourly 12.34
HOURS x 2026-01-05 8.25
HOURS x 2026-01-05 0.75
HOURS x 2026-01-06 7.99
BONUS x 2026-01 0.01
RUN 2026-01
""",
])
def _(s):
    lines = s.split("\n")
    if lines and lines[-1] == "": lines.pop()
    out = []; emps = {}; hours = {}; bonus = {}
    M = r"(?:0|[1-9]\d*)(?:\.\d{1,2})?"
    def date_ok(d):
        try: datetime.date.fromisoformat(d); return True
        except ValueError: return False
    def month_ok(m):
        mm = re.fullmatch(r"(\d{4})-(\d{2})", m); return bool(mm) and 1 <= int(mm[2]) <= 12
    Q = Decimal("0.01")
    for n, line in enumerate(lines, 1):
        if not line.strip(): continue
        w = line.split(" ")
        if line != " ".join(line.split()): out.append(f"line {n}: bad line"); continue
        c = w[0]
        if c == "EMP" and len(w) == 5 and re.fullmatch(r"\w+", w[1]) and re.fullmatch(r"\w+", w[2]) and w[3] in ("hourly", "salary") and re.fullmatch(M, w[4]) and Decimal(w[4]) > 0:
            if w[1] in emps: out.append(f"line {n}: duplicate ID"); continue
            emps[w[1]] = (w[2], w[3], Decimal(w[4]))
        elif c == "HOURS" and len(w) == 4 and re.fullmatch(r"\w+", w[1]) and re.fullmatch(r"\d{4}-\d{2}-\d{2}", w[2]) and date_ok(w[2]) and re.fullmatch(M, w[3]) and Decimal(w[3]) > 0:
            if w[1] not in emps: out.append(f"line {n}: unknown ID"); continue
            if emps[w[1]][1] != "hourly": out.append(f"line {n}: not hourly"); continue
            k = (w[1], w[2]); h = hours.get(k, Decimal(0)) + Decimal(w[3])
            if h > 24: out.append(f"line {n}: too many hours"); continue
            hours[k] = h
        elif c == "BONUS" and len(w) == 4 and re.fullmatch(r"\w+", w[1]) and month_ok(w[2]) and re.fullmatch(M, w[3]) and Decimal(w[3]) > 0:
            if w[1] not in emps: out.append(f"line {n}: unknown ID"); continue
            bonus[(w[1], w[2])] = bonus.get((w[1], w[2]), Decimal(0)) + Decimal(w[3])
        elif c == "RUN" and len(w) == 2 and month_ok(w[1]):
            mo = w[1]; out.append(f"payroll {mo}"); tg = tt = tn = Decimal(0)
            for i in sorted(emps):
                name, kind, r = emps[i]
                if kind == "salary": pay = r / 12
                else:
                    pay = Decimal(0)
                    for (e, d), h in hours.items():
                        if e != i or not d.startswith(mo + "-"): continue
                        if datetime.date.fromisoformat(d).weekday() >= 5: pay += h * r * 2
                        else: pay += min(h, Decimal(8)) * r + max(h - 8, Decimal(0)) * r * Decimal("1.5")
                g = (pay + bonus.get((i, mo), Decimal(0))).quantize(Q, rounding=ROUND_HALF_UP)
                t = (max(Decimal(0), min(g, Decimal(4000)) - 1000) * Decimal("0.2") + max(Decimal(0), g - 4000) * Decimal("0.4")).quantize(Q, rounding=ROUND_HALF_UP)
                out.append(f"  {i} {name} gross {g} tax {t} net {g - t}"); tg += g; tt += t; tn += g - t
            out.append(f"  total gross {d2(tg)} tax {d2(tt)} net {d2(tn)}")
        else:
            out.append(f"line {n}: bad line")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- league
@task("league", """
Keep a football league table. Each line of stdin is a match result `HOME AWAY HG-AG`, for example `Lions Tigers 2-1` (team names are words; goals are whole numbers). Ignore blank lines. Teams join the league the first time they appear.

A win is 3 points, a draw 1, a loss 0. Errors print one line and the result is ignored: `line N: bad line` for a line not in that form (N counts every input line, blank ones included), `line N: same team` when HOME and AWAY are the same, and `line N: already played` when the same HOME and AWAY (in that order) have already played.

After the last line print the table. Teams are ordered by points, then goal difference (goals for minus against), then goals for, all highest first. Teams still level on all three are ordered by the points they won in the matches between themselves only (highest first), and then by name (string order). Print a header line `Pos Team P W D L GF GA GD Pts` and one line per team: position (1, 2, 3, ... with no sharing), name, played, won, drawn, lost, goals for, goals against, goal difference (with a `+` when above 0) and points, separated by single spaces.

Then print `most goals: TEAM (N)` for the team with the most goals for and `best defence: TEAM (N)` for the team with the fewest goals against, taking the first in table order when tied. With no valid results print only `no matches`.
""", [
"""Lions Tigers 2-1
Bears Wolves 0-0
Tigers Bears 3-3
Wolves Lions 1-0
Lions Bears 1-1
Tigers Wolves 2-0
Lions Tigers 0-4
Lions Lions 1-1
lions tigers 1
""",
"""A B 1-0
B C 1-0
C A 1-0
D E 0-0

E D 2-2
""",
"""X Y 1-1
Y X 01-1
X Y -1-0
""",
"""bad
""",
"""Zeta Alpha 1-0
Alpha Cat 1-0
Cat Zeta 1-0
Cat Emu 1-1
""",
])
def _(s):
    lines = s.split("\n")
    if lines and lines[-1] == "": lines.pop()
    out = []; played = set(); teams = []; res = []
    for n, line in enumerate(lines, 1):
        if not line.strip(): continue
        m = re.fullmatch(r"(\w+) (\w+) (0|[1-9]\d*)-(0|[1-9]\d*)", line)
        if not m: out.append(f"line {n}: bad line"); continue
        h, a, hg, ag = m[1], m[2], int(m[3]), int(m[4])
        if h == a: out.append(f"line {n}: same team"); continue
        if (h, a) in played: out.append(f"line {n}: already played"); continue
        played.add((h, a)); res.append((h, a, hg, ag))
        for t in (h, a):
            if t not in teams: teams.append(t)
    if not res:
        out.append("no matches"); return "\n".join(out) + "\n"
    def stats(subset, only):
        st = {t: [0, 0, 0, 0, 0, 0, 0] for t in subset}  # P W D L GF GA Pts
        for h, a, hg, ag in res:
            if only and (h not in subset or a not in subset): continue
            for t, gf, ga in ((h, hg, ag), (a, ag, hg)):
                if t not in st: continue
                x = st[t]; x[0] += 1; x[4] += gf; x[5] += ga
                if gf > ga: x[1] += 1; x[6] += 3
                elif gf == ga: x[2] += 1; x[6] += 1
                else: x[3] += 1
        return st
    st = stats(teams, False)
    key = lambda t: (-st[t][6], -(st[t][4] - st[t][5]), -st[t][4])
    groups = {}
    for t in teams: groups.setdefault(key(t), []).append(t)
    order = []
    for k in sorted(groups):
        g = groups[k]
        if len(g) > 1:
            h2 = stats(g, True); g = sorted(g, key=lambda t: (-h2[t][6], t))
        order += g
    out.append("Pos Team P W D L GF GA GD Pts")
    for i, t in enumerate(order, 1):
        x = st[t]; gd = x[4] - x[5]
        out.append(f"{i} {t} {x[0]} {x[1]} {x[2]} {x[3]} {x[4]} {x[5]} {'+' if gd > 0 else ''}{gd} {x[6]}")
    mg = max(order, key=lambda t: st[t][4]); bd = min(order, key=lambda t: st[t][5])
    out.append(f"most goals: {mg} ({st[mg][4]})")
    out.append(f"best defence: {bd} ({st[bd][5]})")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- crafting
@task("crafting", """
Simulate a crafting workshop. Read commands from stdin, one per line (ignore blank lines):

- `STOCK ITEM QTY`: add QTY (a whole number, at least 1) of ITEM to the stock.
- `RECIPE ITEM N: ING QTY, ING QTY, ...`: one batch of the recipe makes N (at least 1) of ITEM from the listed ingredients (one or more, each with a whole quantity of at least 1, each listed once, none equal to ITEM). Item names are words.
- `CRAFT ITEM QTY`: make QTY of ITEM.
- `SHOW`: print `stock:` and then, in name order, two spaces, the item, a space and the quantity, for every item with more than 0; or `stock: empty`.

CRAFT always runs ITEM's recipe, enough batches to make at least QTY (batches = QTY divided by N, rounded up). For each ingredient, in the order listed, it needs batches times the listed quantity: it takes as much as it can from stock, and if that is not enough it crafts the rest the same way (using that ingredient's recipe, which first takes its own ingredients from stock), or, for an item with no recipe, records a shortage. Everything made beyond what was needed goes into stock; the QTY of ITEM is added to stock too.

If there is any shortage, nothing changes and it prints `line N: short ITEM Q, ITEM Q` with every short item and its total missing quantity, in name order. Otherwise it prints `crafted QTY ITEM (ITEM B, ITEM B)` listing every recipe that ran and its total batches, in name order.

Errors print one line and change nothing: `line N: bad line` for a line not exactly in one of the forms above (N counts every input line, blank ones included), `line N: duplicate recipe` for a second recipe for the same item, `line N: cycle` when the new recipe would make an item need itself (directly or through other recipes), and `line N: no recipe` when CRAFT names an item without a recipe.
""", [
"""STOCK iron 10
STOCK wood 4
RECIPE plank 4: wood 1
RECIPE stick 4: plank 2
RECIPE pick 1: iron 3, stick 2
CRAFT pick 2
SHOW
CRAFT pick 2
SHOW
CRAFT pick 3
SHOW
""",
"""RECIPE a 2: b 1, c 2
RECIPE b 1: c 1
RECIPE c 1: a 1
RECIPE b 3: d 1
RECIPE a 1: a 1
RECIPE e 1: x 0
CRAFT d 1
CRAFT a 3
SHOW
STOCK c 9
CRAFT a 3
SHOW
recipe a 1: b 1
""",
"""SHOW
STOCK gear 2
RECIPE gear 3: bolt 2, plate 1
RECIPE plate 2: ore 3
RECIPE bot 1: gear 4, plate 1, bolt 1
STOCK bolt 20
STOCK ore 3
CRAFT bot 1
STOCK ore 6
CRAFT bot 1
SHOW
CRAFT bot 0
""",
])
def _(s):
    lines = s.split("\n")
    if lines and lines[-1] == "": lines.pop()
    out = []; stock = {}; rec = {}
    def needs(item, target, seen):
        # does `item` need `target` through recipes?
        if item == target: return True
        if item in seen or item not in rec: return False
        seen.add(item)
        return any(needs(i, target, seen) for i, _ in rec[item][1])
    for n, line in enumerate(lines, 1):
        if not line.strip(): continue
        w = line.split(" ")
        if line == "SHOW":
            ks = sorted(k for k, v in stock.items() if v > 0)
            if not ks: out.append("stock: empty")
            else:
                out.append("stock:")
                for k in ks: out.append(f"  {k} {stock[k]}")
            continue
        m = re.fullmatch(r"STOCK (\w+) ([1-9]\d*)", line)
        if m:
            stock[m[1]] = stock.get(m[1], 0) + int(m[2]); continue
        m = re.fullmatch(r"RECIPE (\w+) ([1-9]\d*): (\w+ [1-9]\d*(?:, \w+ [1-9]\d*)*)", line)
        if m:
            ings = [(p.split(" ")[0], int(p.split(" ")[1])) for p in m[3].split(", ")]
            names = [i for i, _ in ings]
            if m[1] in names or len(set(names)) != len(names): out.append(f"line {n}: bad line"); continue
            if m[1] in rec: out.append(f"line {n}: duplicate recipe"); continue
            if any(needs(i, m[1], set()) for i in names): out.append(f"line {n}: cycle"); continue
            rec[m[1]] = (int(m[2]), ings); continue
        m = re.fullmatch(r"CRAFT (\w+) ([1-9]\d*)", line)
        if m:
            item, q = m[1], int(m[2])
            if item not in rec: out.append(f"line {n}: no recipe"); continue
            st = dict(stock); short = {}; batches = {}
            def run(it, qty):
                N, ings = rec[it]; b = -(-qty // N)
                batches[it] = batches.get(it, 0) + b
                for ing, k in ings:
                    need = k * b; take = min(st.get(ing, 0), need)
                    st[ing] = st.get(ing, 0) - take; need -= take
                    if need > 0:
                        if ing in rec:
                            made = run(ing, need); st[ing] = st.get(ing, 0) + made - need
                        else: short[ing] = short.get(ing, 0) + need
                return b * N
            made = run(item, q); st[item] = st.get(item, 0) + made
            if short:
                out.append(f"line {n}: short " + ", ".join(f"{k} {short[k]}" for k in sorted(short))); continue
            stock = st
            out.append(f"crafted {q} {item} (" + ", ".join(f"{k} {batches[k]}" for k in sorted(batches)) + ")")
            continue
        out.append(f"line {n}: bad line")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- tiny_cpu (multi-file)
@task("tiny_cpu", """
Write an interpreter for a tiny assembly language. stdin holds a program, then a line `---`, then input numbers (whole numbers, possibly negative, separated by spaces or newlines).

Each program line is blank, a comment (from `;` to the end of the line; a comment may also follow an instruction), a label `NAME:` (a word, alone on its line), or one instruction. Operands are separated by `, `. A register is `r0` to `r7`; a value is a register or a whole number (possibly negative). Instructions:

- `MOV r, v` / `ADD r, v` / `SUB r, v` / `MUL r, v`: r = v, r + v, r - v, r * v.
- `DIV r, v` / `MOD r, v`: r divided by v rounded toward zero, and the remainder with the sign of r (so `DIV` then `MUL` then `ADD` of the remainder gives r back).
- `JMP L`, `JZ r, L` (jump if r is 0), `JNZ r, L`, `JLT v, v, L` (jump if the first value is less than the second).
- `CALL L` pushes the return position on the call stack and jumps; `RET` returns to it.
- `PUSH v` and `POP r` use a data stack (separate from the call stack).
- `READ r` sets r to the next input number. `PRINT v` prints the value on its own line.
- `HALT` stops.

All registers start at 0. The program also stops when it runs past its last instruction.

Before running, check the whole program. Each bad line prints `line N: bad instruction` (an unknown instruction, wrong number or kind of operands, or a malformed label); a jump or call to a label that does not exist prints `line N: unknown label L`; a label defined twice prints `line N: duplicate label L` at its second definition. N is the line number in stdin. If any of these occur, print them in line order and do not run.

While running, these stop the program with an error line: `error line N: division by zero`, `error line N: stack empty` (POP or RET with nothing on its stack), `error line N: no input` (READ with no numbers left), and `error: too many steps` after 10000 instructions have run without stopping.

Finally (also after a run-time error) print `steps S` (the number of instructions that ran, counting the one that failed) and then `regs` followed by ` rI=V` for each register that is not 0, in order, or `regs none`.
""", [
"""; sum the numbers until a 0, then print the count and the sum
    READ r1
loop:
    JZ r1, done
    ADD r2, r1
    ADD r3, 1
    READ r1
    JMP loop
done:
    PRINT r3
    PRINT r2
    HALT
---
4 -7 10
0 99
""",
"""    READ r0
    CALL fact
    PRINT r1
    MOV r2, -7
    MOV r3, r2
    DIV r2, 2   ; -3
    MOD r3, 2   ; -1
    PRINT r2
    PRINT r3
    MOV r4, 3
    PUSH r4
    PUSH 5
    POP r5
    POP r6
    POP r7
fact:
    MOV r1, 1
again:
    JLT r0, 2, back
    MUL r1, r0
    SUB r0, 1
    JMP again
back:
    RET
---
6
""",
"""start:
    MOV r1, 5
    mov r2, 1
    ADD r9, 1
    JMP nowhere
start:
    PRINT r1 r2
    HALT 3
1bad:
    DIV r1
---
""",
"""    MOV r1, 10
    DIV r1, r2
---
""",
"""top:
    ADD r0, 1
    JMP top
---
""",
"""    READ r1
    READ r2
---
5
""",
], files=["main", "parse", "machine"])
def _(s):
    lines = s.split("\n")
    if lines and lines[-1] == "": lines.pop()
    sep = lines.index("---")
    prog, rest = lines[:sep], lines[sep + 1:]
    nums = [int(x) for x in " ".join(rest).split()]
    R = r"r[0-7]"; V = r"(?:r[0-7]|-?\d+)"; L = r"[A-Za-z_]\w*"
    forms = {"MOV": [R, V], "ADD": [R, V], "SUB": [R, V], "MUL": [R, V], "DIV": [R, V], "MOD": [R, V],
             "JMP": [L], "JZ": [R, L], "JNZ": [R, L], "JLT": [V, V, L], "CALL": [L], "RET": [],
             "PUSH": [V], "POP": [R], "READ": [R], "PRINT": [V], "HALT": []}
    errs = []; code = []; labels = {}; refs = []
    for n, raw in enumerate(prog, 1):
        line = raw.split(";")[0].strip()
        if not line: continue
        if line.endswith(":"):
            name = line[:-1]
            if not re.fullmatch(L, name): errs.append((n, "bad instruction")); continue
            if name in labels: errs.append((n, f"duplicate label {name}")); continue
            labels[name] = len(code); continue
        m = re.fullmatch(r"([A-Z]+)(?: (.*))?", line)
        if not m or m[1] not in forms: errs.append((n, "bad instruction")); continue
        ops = m[2].split(", ") if m[2] is not None else []
        f = forms[m[1]]
        if len(ops) != len(f) or not all(re.fullmatch(p, o) for p, o in zip(f, ops)): errs.append((n, "bad instruction")); continue
        if f and f[-1] == L: refs.append((n, ops[-1]))
        code.append((n, m[1], ops))
    for n, l in refs:
        if l not in labels: errs.append((n, f"unknown label {l}"))
    if errs:
        return "\n".join(f"line {n}: {e}" for n, e in sorted(errs)) + "\n"
    reg = [0] * 8; data = []; calls = []; pc = 0; steps = 0; out = []
    val = lambda o: reg[int(o[1])] if o.startswith("r") else int(o)
    while pc < len(code):
        if steps == 10000: out.append("error: too many steps"); break
        n, op, a = code[pc]; steps += 1; pc += 1
        if op in ("MOV", "ADD", "SUB", "MUL", "DIV", "MOD"):
            r = int(a[0][1]); v = val(a[1])
            if op == "MOV": reg[r] = v
            elif op == "ADD": reg[r] += v
            elif op == "SUB": reg[r] -= v
            elif op == "MUL": reg[r] *= v
            else:
                if v == 0: out.append(f"error line {n}: division by zero"); break
                q = abs(reg[r]) // abs(v) * (1 if (reg[r] >= 0) == (v > 0) else -1)
                reg[r] = q if op == "DIV" else reg[r] - q * v
        elif op == "JMP": pc = labels[a[0]]
        elif op == "JZ":
            if reg[int(a[0][1])] == 0: pc = labels[a[1]]
        elif op == "JNZ":
            if reg[int(a[0][1])] != 0: pc = labels[a[1]]
        elif op == "JLT":
            if val(a[0]) < val(a[1]): pc = labels[a[2]]
        elif op == "CALL": calls.append(pc); pc = labels[a[0]]
        elif op == "RET":
            if not calls: out.append(f"error line {n}: stack empty"); break
            pc = calls.pop()
        elif op == "PUSH": data.append(val(a[0]))
        elif op == "POP":
            if not data: out.append(f"error line {n}: stack empty"); break
            reg[int(a[0][1])] = data.pop()
        elif op == "READ":
            if not nums: out.append(f"error line {n}: no input"); break
            reg[int(a[0][1])] = nums.pop(0)
        elif op == "PRINT": out.append(str(val(a[0])))
        elif op == "HALT": break
    out.append(f"steps {steps}")
    nz = [f"r{i}={v}" for i, v in enumerate(reg) if v != 0]
    out.append("regs " + (" ".join(nz) if nz else "none"))
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- parking (multi-file)
@task("parking", """
Run a car park. The first line of stdin is `spaces car C moto M van V` (whole numbers, 0 or more). Every later line is `DATE TIME COMMAND` with DATE as `YYYY-MM-DD` and TIME as `HH:MM` (24-hour), or blank (ignore it). Commands:

- `IN PLATE TYPE`: a vehicle arrives. PLATE is a word, TYPE is `car`, `moto` or `van`.
- `OUT PLATE`: it leaves and pays.
- `REPORT`: print the state.

Spaces are named by type letter and number: `C1`..`CC`, `M1`..`MM`, `V1`..`VV`. A car takes the lowest-numbered free car space; a van the lowest free van space; a moto the lowest free moto space, or if none is free, the lowest free car space. On success print `IN PLATE SPACE`; if there is no space for it print `FULL PLATE` (it does not enter).

On OUT, the fee depends on TYPE (not the space used): car 2.50 per started hour with a cap of 20.00 per day, moto 1.20 per hour with a cap of 8.00, van 4.00 per hour with a cap of 30.00. For a stay of D minutes: a stay of 15 minutes or less is free; otherwise split the stay into whole days of 24 hours and a rest; the fee is days times the cap, plus the rest's started hours times the rate but at most the cap. Print `OUT PLATE HhMMm fee F` with the stay as hours and two-digit minutes (`26h05m`) and the fee with 2 decimals.

`REPORT` prints `occupied car X/C moto Y/M van Z/V revenue R`, counting the vehicles in each kind of space (a moto in a car space counts as car), and the fees taken so far.

Errors print one line and change nothing: `line N: bad line` for a line not exactly in one of the forms above, with a real date and time (N counts every input line, blank ones included), `line N: time goes back` when the time is before the previous accepted line's time, `line N: already inside` for IN of a plate that is inside, and `line N: not inside` for OUT of a plate that is not.

After the last line print `revenue R from K exits`.
""", [
"""spaces car 2 moto 1 van 1
2026-03-01 08:00 IN AB12 car
2026-03-01 08:05 IN M1 moto
2026-03-01 08:06 IN M2 moto
2026-03-01 08:07 IN CC car
2026-03-01 08:10 IN VV van
2026-03-01 08:11 IN V2 van
2026-03-01 08:20 OUT M2
2026-03-01 09:00 REPORT
2026-03-01 10:01 OUT AB12
2026-03-02 10:21 OUT VV
2026-03-03 07:00 OUT M1
2026-03-03 07:01 OUT M1
2026-03-03 07:02 REPORT
""",
"""spaces car 1 moto 0 van 0
2026-02-28 23:50 IN X car
2026-02-28 23:40 IN Y car
2026-02-29 00:10 OUT X
2026-03-01 00:05 IN X car
2026-03-01 00:06 IN X car
2026-03-01 25:00 OUT X
2026-03-01 09:00 OUT X
2026-03-01 09:00 IN Z truck
2026-03-01 09:00 in Z car

2026-03-01 09:30 IN Z moto
2026-03-01 09:44 OUT Z
""",
"""spaces car 0 moto 1 van 0
2026-01-01 00:00 IN m moto
2026-01-01 00:01 IN c car
2026-01-04 00:00 OUT m
2026-01-04 00:00 REPORT
""",
], files=["main", "garage", "billing"])
def _(s):
    lines = s.split("\n")
    if lines and lines[-1] == "": lines.pop()
    m = re.fullmatch(r"spaces car (\d+) moto (\d+) van (\d+)", lines[0])
    cap = {"C": int(m[1]), "M": int(m[2]), "V": int(m[3])}
    used = {"C": set(), "M": set(), "V": set()}
    inside = {}; last = None; rev = Decimal(0); exits = 0; out = []
    RATE = {"car": (Decimal("2.50"), Decimal("20.00")), "moto": (Decimal("1.20"), Decimal("8.00")), "van": (Decimal("4.00"), Decimal("30.00"))}
    for n, line in enumerate(lines[1:], 2):
        if not line.strip(): continue
        mm = re.fullmatch(r"(\d{4}-\d{2}-\d{2}) (\d{2}):(\d{2}) (?:(IN) (\w+) (car|moto|van)|(OUT) (\w+)|(REPORT))", line)
        t = None
        if mm:
            try:
                d = datetime.date.fromisoformat(mm[1]); hh, mi = int(mm[2]), int(mm[3])
                if hh < 24 and mi < 60: t = datetime.datetime(d.year, d.month, d.day, hh, mi)
            except ValueError: pass
        if t is None: out.append(f"line {n}: bad line"); continue
        if last and t < last: out.append(f"line {n}: time goes back"); continue
        if mm[4]:
            p, ty = mm[5], mm[6]
            if p in inside: out.append(f"line {n}: already inside"); continue
            last = t
            kinds = {"car": ["C"], "van": ["V"], "moto": ["M", "C"]}[ty]; sp = None
            for k in kinds:
                free = [i for i in range(1, cap[k] + 1) if i not in used[k]]
                if free: sp = (k, free[0]); break
            if not sp: out.append(f"FULL {p}"); continue
            used[sp[0]].add(sp[1]); inside[p] = (ty, sp, t); out.append(f"IN {p} {sp[0]}{sp[1]}")
        elif mm[7]:
            p = mm[8]
            if p not in inside: out.append(f"line {n}: not inside"); continue
            last = t
            ty, sp, t0 = inside.pop(p); used[sp[0]].discard(sp[1])
            D = int((t - t0).total_seconds() // 60)
            rate, cp = RATE[ty]
            if D <= 15: fee = Decimal(0)
            else:
                days, rest = divmod(D, 1440)
                fee = days * cp + min(cp, -(-rest // 60) * rate)
            rev += fee; exits += 1
            out.append(f"OUT {p} {D // 60}h{D % 60:02d}m fee {d2(fee)}")
        else:
            last = t
            out.append(f"occupied car {len(used['C'])}/{cap['C']} moto {len(used['M'])}/{cap['M']} van {len(used['V'])}/{cap['V']} revenue {d2(rev)}")
    out.append(f"revenue {d2(rev)} from {exits} exits")
    return "\n".join(out) + "\n"

ROLES = {
    "tiny_cpu": "`parse` reads and checks the program (instructions, operands, labels) and reports the check errors; `machine` holds the registers and stacks and runs the checked program; `main` splits stdin, calls them and prints the result.",
    "parking": "`garage` tracks the spaces and the vehicles inside; `billing` works out stay lengths and fees; `main` reads and checks the lines, keeps the clock and revenue, and prints.",
}
for name, (prompt, inputs, fn) in T.items():
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
