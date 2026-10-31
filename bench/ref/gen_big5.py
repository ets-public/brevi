# Held-out XL tasks, third set (written 2026-10-31, after the first v0.19 fixes; never used to tune the language).
# Same size as gen_big3.py and gen_big4.py (150 to 400 lines); two of them must be split over several files.
# Writes tasks/<name>/{prompt.md,tests/N.in,N.out} and, for multi-file tasks, tasks/<name>/files.
import os, re, datetime, itertools
from decimal import Decimal, ROUND_HALF_UP, InvalidOperation
T = {}
FILES = {}
def task(name, prompt, inputs, files=None):
    def deco(fn):
        T[name] = (prompt.strip(), inputs, fn)
        if files: FILES[name] = files
        return fn
    return deco

def d2(x): return str(Decimal(x).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))
def lines_of(s):
    ls = s.split("\n")
    if ls and ls[-1] == "": ls.pop()
    return ls
def date_of(s):
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", s): return None
    try: return datetime.date.fromisoformat(s)
    except ValueError: return None

MULTI = """
Write the program as {n} files in the folder you are given: {files}. `main` is the entry point; each file imports what it needs from the others. {roles}
"""

# ---------------------------------------------------------------- books
@task("books", r"""
Write a double-entry bookkeeping checker. Read lines from stdin, numbered from 1.

- `ACCOUNT NAME TYPE` opens an account. TYPE is one of `asset`, `liability`, `equity`, `income`, `expense`. A second ACCOUNT line with a name already opened prints `line N: duplicate account` and is ignored.
- `TX DATE DESCRIPTION...` starts a transaction. DATE is `YYYY-MM-DD` and must be a real date; the description is one or more words.
- A line that starts with two spaces is a posting of the transaction above it: `  NAME AMOUNT`, where AMOUNT is a signed decimal with at most 2 decimal places (`120`, `-45.5`, `+3.25`). Positive amounts are debits, negative amounts are credits.
- `REPORT` prints the trial balance (below).
- Blank lines are ignored and end nothing. Any other line, or a line of the kinds above with the wrong number of words or a bad value, prints `line N: bad line` and is ignored. A posting line with no transaction open (before any TX, or after a line that is not a posting) is a bad line too.

A transaction ends at the next line that is not a posting (or at the end of input), and is then checked, in this order:
1. If its TX line was a bad line, the transaction and its postings are ignored silently.
2. A posting line that is a bad line, or names an account not opened yet (`line N: unknown account NAME`), is reported when read; the transaction is then discarded with no further message.
3. If its date is before the date of the last accepted transaction, print `line N: date goes back` (N = the TX line) and discard it.
4. With fewer than two postings, print `tx on line N: needs two postings` and discard it.
5. If the amounts do not add up to zero, print `tx on line N: unbalanced by X`, where X is the sum with 2 decimals and a sign (`+12.50`, `-0.01`), and discard it.
Otherwise the transaction is accepted and its postings are added to the account balances.

A REPORT line (which also ends an open transaction, checked first) prints `trial balance` and then one line per opened account: `NAME TYPE BALANCE`. Accounts are ordered by type (asset, liability, equity, income, expense), then by name. BALANCE has 2 decimals: for asset and expense accounts it is the sum of their postings; for the other types it is minus that sum, so a normal balance is positive either way. Then print `debits D credits C`: the totals of all positive and of all negative accepted amounts (C without its sign), with 2 decimals.

At the end print `K transactions accepted, R rejected`, where R counts transactions discarded with a message or for a bad posting (not those with a bad TX line).
""", [
"""ACCOUNT cash asset
ACCOUNT bank asset
ACCOUNT sales income
ACCOUNT rent expense
ACCOUNT loan liability
ACCOUNT capital equity
TX 2026-01-02 owner puts money in
  bank 5000
  capital -5000
TX 2026-01-05 cash sale
  cash 120.50
  sales -120.50
TX 2026-01-06 pay rent
  rent 800
  bank -800
REPORT
TX 2026-01-07 take loan
  bank 2000
  loan -2000
REPORT
""",
"""ACCOUNT cash asset
ACCOUNT sales income
ACCOUNT cash asset
ACCOUNT fees expense
ACCOUNT wages cost
TX 2026-03-01 sale
  cash 10
  sales -9.99
TX 2026-03-02 single leg
  cash 5
TX 2026-02-28 too early
  cash 1
  sales -1
TX 2026-03-03 unknown
  cash 4
  bonus -4
TX 2026-03-04 fine
  cash 7.25
  sales -7
  fees -0.25
  cash 1.234
TX 2026-03-05 fine again
  cash +3
  sales -3
  fees 0

TX 2026-02-30 bad date
  cash 1
  sales -1
  cash 2
REPORT
""",
"""  cash 10
ACCOUNT a asset
ACCOUNT b liability
TX 2026-05-01
  a 1
  b -1
TX 2026-05-01 ok
  a 1
REPORT extra
  b -1
TX 2026-05-01 later
  a 2.5
  b -2.5
  a
REPORT
""",
"""ACCOUNT z expense
ACCOUNT y asset
ACCOUNT x income
REPORT
TX 2026-07-01 three way
  y 100
  x -60
  x -40
TX 2026-07-01 refund
  x 15.75
  y -15.75
TX 2026-07-02 costs
  z 20
  y -20
TX 2026-07-01 late
  z 1
  y -1
REPORT
""",
], files=None)
def _(s):
    TYPES = ["asset", "liability", "equity", "income", "expense"]
    acc = {}; out = []; last = None; tx = None; acc_n = rej = 0
    deb = Decimal(0); cred = Decimal(0)
    def close():
        nonlocal tx, last, acc_n, rej, deb, cred
        if tx is None: return
        t = tx; tx = None
        if t["bad"]: return
        if t["badpost"]: rej += 1; return
        if last is not None and t["date"] < last:
            out.append(f"line {t['line']}: date goes back"); rej += 1; return
        if len(t["posts"]) < 2:
            out.append(f"tx on line {t['line']}: needs two postings"); rej += 1; return
        tot = sum(a for _, a in t["posts"])
        if tot != 0:
            q = d2(tot); q = q if q.startswith("-") else "+" + q
            out.append(f"tx on line {t['line']}: unbalanced by {q}"); rej += 1; return
        for n, a in t["posts"]:
            acc[n][1] += a
            if a > 0: deb += a
            else: cred -= a
        last = t["date"]; acc_n += 1
    for n, raw in enumerate(lines_of(s), 1):
        if raw.startswith("  "):
            w = raw.split()
            if tx is None:
                out.append(f"line {n}: bad line"); continue
            if len(w) != 2 or not re.fullmatch(r"[+-]?\d+(\.\d{1,2})?", w[1]):
                out.append(f"line {n}: bad line"); tx["badpost"] = True; continue
            if w[0] not in acc:
                out.append(f"line {n}: unknown account {w[0]}"); tx["badpost"] = True; continue
            tx["posts"].append((w[0], Decimal(w[1]))); continue
        w = raw.split()
        if not w: continue
        close()
        if w[0] == "ACCOUNT" and len(w) == 3 and w[2] in TYPES:
            if w[1] in acc: out.append(f"line {n}: duplicate account")
            else: acc[w[1]] = [w[2], Decimal(0)]
        elif w[0] == "TX" and len(w) >= 3 and date_of(w[1]):
            tx = {"line": n, "date": date_of(w[1]), "posts": [], "bad": False, "badpost": False}
        elif w[0] == "TX":
            out.append(f"line {n}: bad line"); tx = {"line": n, "bad": True, "posts": [], "badpost": False}
        elif w == ["REPORT"]:
            out.append("trial balance")
            for name in sorted(acc, key=lambda a: (TYPES.index(acc[a][0]), a)):
                ty, b = acc[name]
                if ty not in ("asset", "expense"): b = -b
                out.append(f"{name} {ty} {d2(b)}")
            out.append(f"debits {d2(deb)} credits {d2(cred)}")
        else:
            out.append(f"line {n}: bad line")
    close()
    out.append(f"{acc_n} transactions accepted, {rej} rejected")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- toc
@task("toc", r"""
Write a table-of-contents builder for a Markdown document read from stdin (lines numbered from 1).

A heading is a line that starts with 1 to 6 `#` characters followed by a space and at least one non-space character. Its level is the number of `#`. Its text is the rest of the line with surrounding spaces removed and, if the text ends with a space followed by one or more `#`, those `#` and the spaces before them removed too. `#tag`, `####### seven` and `#` alone are not headings. Lines inside fenced code blocks are never headings: a line starting with three backticks opens a block, and the next line starting with three backticks closes it.

Numbering: the top level is the smallest heading level in the document. Each heading gets a depth: the first heading has depth 1 + (its level - top level); after that, a heading whose level is deeper than the previous heading's level by more than one prints a warning `line N: level jump` and gets depth previous depth + 1; otherwise its depth is 1 + (its level - top level). Number headings like `1`, `1.1`, `1.2`, `2`, `2.1.1`: a heading at depth d increments the counter at depth d and resets all deeper counters, and its number is the counters from depth 1 to d joined by `.`. A counter that was never set prints as 0, so a document whose first heading is below the top level starts at `0.1`.

Slug: the text lowercased, keeping only letters a-z, digits, spaces and `-`, then each space replaced by `-`. An empty slug becomes `section`. If the same slug was already used, add `-1` for its second use, `-2` for the third, and so on. Only the plain slugs are counted: a later heading `A-1` still gets `a-1` even if `a-1` was already given to the second `A`.

Output: first every warning, in line order. A code block still open at the end of input adds the warning `line N: unclosed code block` (N = its opening line). Then one line per heading: two spaces per depth level beyond 1, then `NUMBER TEXT (#SLUG)`. Last, `K headings, max depth D`. If there are no headings, print the warnings and then `no headings`.
""", [
"""# Guide
Intro text.
## Install
### From source ###
## Usage
Some `#notes` here.
#hashtag is not a heading
## Usage
# FAQ
""",
"""Text before.
### Deep start
#### Child
###### Too deep
## Shallower
```
# not a heading
```
## Why? (and how!)
##     Spaced    ##
## ???
""",
"""```python
# comment
## also comment
""",
"""## A
## a
## A-1
#### Jump
### Back
####### seven
#
# Last #tag
""",
], files=None)
def _(s):
    heads = []; warns = []; fence = None
    for n, raw in enumerate(lines_of(s), 1):
        if raw.startswith("```"):
            fence = None if fence else n; continue
        if fence: continue
        m = re.match(r"(#{1,6}) (.*)$", raw)
        if not m or not m[2].strip(): continue
        text = m[2].strip()
        text = re.sub(r"\s+#+$", "", text).strip() if re.search(r"\s#+$", text) else text
        heads.append((n, len(m[1]), text))
    if fence: warns.append((fence, f"line {fence}: unclosed code block"))
    if not heads:
        return "".join(w + "\n" for _, w in sorted(warns)) + "no headings\n"
    top = min(l for _, l, _ in heads); out = []; cnt = []; prev = None; used = {}; maxd = 0
    for n, lv, text in heads:
        if prev is None: d = 1 + lv - top
        elif lv > prev[0] + 1: warns.append((n, f"line {n}: level jump")); d = prev[1] + 1
        else: d = 1 + lv - top
        while len(cnt) < d: cnt.append(0)
        cnt = cnt[:d]; cnt[d - 1] += 1
        num = ".".join(str(c) for c in cnt)
        slug = re.sub(r"[^a-z0-9 -]", "", text.lower()).replace(" ", "-") or "section"
        if slug in used: used[slug] += 1; slug = f"{slug}-{used[slug]}"
        else: used[slug] = 0
        out.append("  " * (d - 1) + f"{num} {text} (#{slug})")
        maxd = max(maxd, d); prev = (lv, d)
    res = [w for _, w in sorted(warns)] + out + [f"{len(heads)} headings, max depth {maxd}"]
    return "\n".join(res) + "\n"

# ---------------------------------------------------------------- poker
@task("poker", r"""
Write a poker hand ranker. Each line of stdin (numbered from 1) is `NAME C1 C2 C3 C4 C5`: a player and five cards. A card is a rank (`2`-`9`, `T`, `J`, `Q`, `K`, `A`) followed by a suit (`S`, `H`, `D`, `C`). A blank line ends a round; the input's end ends the last round (a round with no lines at all after a blank line is not a round).

Checks, in order, for each line: anything but a name and five well-formed cards, or the same card twice in the hand, prints `line N: bad hand`; a name already playing in this round prints `line N: duplicate player`; a card already dealt to an earlier player in this round prints `line N: card XX already dealt` (the first such card in the hand). Such lines are ignored.

Hand categories, best first: straight flush, four of a kind, full house, flush, straight, three of a kind, two pair, pair, high card. A straight is five consecutive ranks; A-2-3-4-5 is a straight whose high card is 5 (and also a straight flush when suited). Hands of the same category compare by their ranks in this order: four of a kind by the quad then the kicker; full house by the triple then the pair; three of a kind by the triple then the remaining cards high to low; two pair by the higher pair, the lower pair, then the kicker; pair by the pair then the rest high to low; flush and high card by all five cards high to low; straights by their high card. Suits never break ties.

For each round print `round R` (counting rounds from 1) and then each player best first as `P. NAME CATEGORY`. Tied hands share the position number and are listed by name; the next position skips (1, 1, 3). A round with no valid players prints `round R` and `no players`.

At the end print `R rounds`.
""", [
"""ann AS KS QS JS TS
bob 2H 2D 2C KD KH
cat 9C 9D 4S 4H AC
dan 3D 4D 5D 6D 8D
""",
"""eve AH 2D 3C 4S 5H
fay 6C 7C 8D 9S TD
gus KH KC 7D 7S 2C
hal KD KS 7H 7C 3D

ivy 8S 8H 4D 3C 2S
jay 8D 8C 4H 3S 2D
kim AD AC QH JH 9S
""",
"""lee AS AS KD QH JC
mo 1S 2S 3S 4S 5S
ned TH JH QH KH AH
ned 2C 3C 4C 5C 7D
oz AH 9C 8C 7C 6C
pat 5C 6H 7S 8H 9D

""",
"""zed 2C 3C 4C 5C AC
yan 5S 5H 5D 5C 2D
xu QD QH 3S 3D 3H
wil 2D 7S 9H JC KS
vic 2H 7C 9D JS KH
""",
], files=None)
def _(s):
    R = "23456789TJQKA"; CAT = ["high card", "pair", "two pair", "three of a kind", "straight", "flush", "full house", "four of a kind", "straight flush"]
    def score(cards):
        rs = sorted((R.index(c[0]) for c in cards), reverse=True)
        fl = len({c[1] for c in cards}) == 1
        st = None
        if len(set(rs)) == 5:
            if rs[0] - rs[4] == 4: st = rs[0]
            elif rs == [12, 3, 2, 1, 0]: st = 3
        cnt = sorted(((rs.count(r), r) for r in set(rs)), reverse=True)
        shape = [c for c, _ in cnt]; order = [r for _, r in cnt]
        if st is not None and fl: return (8, [st])
        if shape == [4, 1]: return (7, order)
        if shape == [3, 2]: return (6, order)
        if fl: return (5, rs)
        if st is not None: return (4, [st])
        if shape == [3, 1, 1]: return (3, order)
        if shape == [2, 2, 1]: return (2, order)
        if shape == [2, 1, 1, 1]: return (1, order)
        return (0, rs)
    rounds = []; cur = None
    out = []
    def finish():
        nonlocal cur
        if cur is None: return
        out.append(f"round {len(rounds) + 1}")
        rounds.append(1)
        if not cur["players"]: out.append("no players")
        else:
            ps = sorted(cur["players"], key=lambda p: (tuple([-p[1][0]] + [-x for x in p[1][1]]), p[0]))
            pos = 0
            for i, p in enumerate(ps):
                if i == 0 or p[1] != ps[i - 1][1]: pos = i + 1
                out.append(f"{pos}. {p[0]} {CAT[p[1][0]]}")
        cur = None
    for n, raw in enumerate(lines_of(s), 1):
        w = raw.split()
        if not w:
            finish(); continue
        if cur is None: cur = {"players": [], "names": set(), "dealt": set()}
        msgs = []
        if len(w) != 6 or not all(re.fullmatch(r"[2-9TJQKA][SHDC]", c) for c in w[1:]) or len(set(w[1:])) != 5:
            out.append(f"line {n}: bad hand"); continue
        if w[0] in cur["names"]:
            out.append(f"line {n}: duplicate player"); continue
        dup = next((c for c in w[1:] if c in cur["dealt"]), None)
        if dup:
            out.append(f"line {n}: card {dup} already dealt"); continue
        cur["names"].add(w[0]); cur["dealt"] |= set(w[1:])
        cur["players"].append((w[0], score(w[1:])))
    finish()
    out.append(f"{len(rounds)} rounds")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- trains
@task("trains", r"""
Write a journey planner for a timetable read from stdin (lines numbered from 1). Times are `HH:MM` on one day (00:00 to 23:59).

- `TRAIN ID STOP TIME STOP TIME ...` defines a train with at least two stops: it is at each stop at the given time, in order. Times must strictly increase and a stop may not appear twice; otherwise print `line N: bad train`. A second train with an ID already used prints `line N: duplicate train`. Bad and duplicate trains are ignored.
- `TRIP FROM TO TIME` asks for the earliest arrival at TO for a traveller standing at FROM at TIME. It only uses trains defined above it.
- Blank lines are ignored; anything else, or a TRAIN or TRIP line with the wrong shape or a bad time, prints `line N: bad line`.

For a trip: a traveller can board a train at any of its stops at or after the moment they are there, ride it, and get off at any later stop of that train. Changing to a different train at a stop needs at least 2 minutes between arriving and the next train's time at that stop; at the starting stop they can board any train whose time there is at or after TIME. Staying on the same train is not a change.

Print `TRIP FROM->TO: arrive HH:MM via A,B,...` where the list is the trains used, in order, one entry per ride. Choose the plan with the earliest arrival; among those, the fewest rides; among those, the one whose list of train IDs is smallest comparing ID by ID as text. If FROM or TO is not a stop of any train defined so far, print `TRIP FROM->TO: unknown stop NAME` (FROM is checked first). If FROM equals TO, print `TRIP FROM->TO: already there`. If no plan reaches TO, print `TRIP FROM->TO: no route`.

At the end print `T trains, Q trips`, counting accepted trains and all well-formed TRIP lines.
""", [
"""TRAIN r1 a 08:00 b 08:20 c 08:45 d 09:10
TRAIN r2 b 08:30 e 08:50 d 09:00
TRAIN r3 c 08:46 d 09:05
TRIP a d 07:50
TRIP a d 08:01
TRIP b d 08:21
TRIP e a 08:00
TRIP x a 08:00
""",
"""TRAIN s1 p 10:00 q 10:30
TRAIN s2 q 10:31 r 11:00
TRAIN s3 q 10:32 r 11:00
TRAIN s4 q 10:40 r 10:59
TRIP p r 09:00
TRAIN s0 p 09:00 r 12:00
TRIP p r 08:59
TRIP q q 10:00
TRIP p z 10:00
""",
"""TRAIN bad1 a 10:00 b 09:59
TRAIN bad2 a 10:00 a 10:30
TRAIN t1 a 25:00 b 26:00
TRAIN t2 a 10:00
TRAIN ok a 10:00 b 10:10
TRAIN ok c 10:00 d 10:10
TRIP a b
TRIP a b 9:5
TRIP b a 10:00
""",
"""TRAIN k2 m 06:00 n 06:30 o 07:00
TRAIN k1 m 06:00 n 06:30 o 07:00
TRAIN k3 n 06:32 o 06:50
TRAIN k4 n 06:31 o 06:50
TRIP m o 05:00
TRIP n o 06:30
TRIP m n 06:00
""",
], files=None)
def _(s):
    trains = {}; out = []; ntrip = 0
    def tm(x):
        m = re.fullmatch(r"(\d\d):(\d\d)", x)
        if not m or int(m[1]) > 23 or int(m[2]) > 59: return None
        return int(m[1]) * 60 + int(m[2])
    def fmt(t): return f"{t // 60:02d}:{t % 60:02d}"
    for n, raw in enumerate(lines_of(s), 1):
        w = raw.split()
        if not w: continue
        if w[0] == "TRAIN" and len(w) >= 6 and len(w) % 2 == 0:
            stops = w[2::2]; times = [tm(x) for x in w[3::2]]
            if None in times: out.append(f"line {n}: bad line"); continue
            if any(times[i] >= times[i + 1] for i in range(len(times) - 1)) or len(set(stops)) != len(stops):
                out.append(f"line {n}: bad train"); continue
            if w[1] in trains: out.append(f"line {n}: duplicate train"); continue
            trains[w[1]] = list(zip(stops, times))
        elif w[0] == "TRIP" and len(w) == 4 and tm(w[3]) is not None:
            ntrip += 1; a, b, t0 = w[1], w[2], tm(w[3])
            head = f"TRIP {a}->{b}: "
            allstops = {st for tr in trains.values() for st, _ in tr}
            if a not in allstops: out.append(head + f"unknown stop {a}"); continue
            if b not in allstops: out.append(head + f"unknown stop {b}"); continue
            if a == b: out.append(head + "already there"); continue
            best = None
            # Search all plans: state (stop, time, last train, rides list).
            def go(stop, t, last, rides, seen):
                nonlocal best
                if stop == b:
                    k = (t, len(rides), rides)
                    if best is None or k < best: best = k
                    return
                for tid, tr in sorted(trains.items()):
                    if tid in seen: continue
                    for i, (st, tt) in enumerate(tr):
                        if st != stop: continue
                        need = t if last is None else t + 2
                        if tt < need: continue
                        for st2, tt2 in tr[i + 1:]:
                            go(st2, tt2, tid, rides + [tid], seen | {tid})
            go(a, t0, None, [], frozenset())
            if best is None: out.append(head + "no route")
            else: out.append(head + f"arrive {fmt(best[0])} via {','.join(best[2])}")
        else:
            out.append(f"line {n}: bad line")
    out.append(f"{len(trains)} trains, {ntrip} trips")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- quiz (multi-file)
@task("quiz", r"""
Write a quiz grader. Stdin (lines numbered from 1) has a line `KEY`, then answer-key lines, then a line `ANSWERS`, then student lines. Blank lines are ignored everywhere. Lines before `KEY`, a second `KEY` or `ANSWERS`, or a missing section are not tested.

Key lines are `ID TYPE POINTS SPEC`, where ID is `Q` followed by a whole number, POINTS a whole number from 1 to 100, and:
- `choice POINTS X`: one letter A-E. Full points for exactly that letter.
- `multi POINTS LETTERS`: two or more different letters A-E, written together (`ACD`). A student's answer is a set of letters (repeats count once). Full points for exactly the set; otherwise points * (right letters chosen - wrong letters chosen) / (letters in the key), rounded down, and never below 0.
- `number POINTS VALUE TOL`: VALUE and TOL decimals (TOL not negative). Full points if the answer is a decimal within TOL of VALUE (inclusive).
- `text POINTS WORD|WORD|...`: one or more words separated by `|`. Full points if the answer equals one of them ignoring case.
A key line with any other shape, or an ID used before, prints `key line N: bad question` and that line is ignored.

Student lines are `NAME ID=ANSWER ID=ANSWER ...` (zero or more answers; an answer has no spaces). A NAME used before prints `line N: duplicate student` and the line is ignored. Within an accepted line: a token without `=` or with an empty answer prints `line N: bad answer TOKEN`; an ID not in the key prints `line N: unknown question ID`; a second answer to the same question prints `line N: answered twice ID`. These answers are skipped; the rest of the line still counts. Unanswered questions score 0, and so does an answer of the wrong form (a `number` answer that is not a decimal, `choice` or `multi` answers with letters outside A-E).

Output, in this order:
1. Messages, in line order, as they occur.
2. For each student in input order: `NAME SCORE/MAX PCT%`, where MAX is the sum of all key points and PCT is SCORE * 100 / MAX rounded down.
3. For each key question in key order: `ID avg X` with X the mean score of the students, 2 decimals (half up).
4. `top: NAMES (SCORE)`: the highest score and every student with it, names in input order separated by `, `.
If there are no students, print the messages and then only `no students`. If the key has no valid question, print the messages and then only `empty key`.
""", [
"""KEY
Q1 choice 2 B
Q2 multi 4 ACD
Q3 number 3 3.14 0.01
Q4 text 1 paris|Paris|PARIS
Q5 text 2 Lyon
ANSWERS
amy Q1=B Q2=ACD Q3=3.15 Q4=paris Q5=lyon
ben Q1=C Q2=AC Q3=3.16 Q4=Rome
cal Q2=ADBE Q3=x Q5=LYON Q1=B
dee
""",
"""KEY
Q1 choice 5 F
Q1 choice 5 A
Q2 number 2 10 -1
Q2 number 2 10 0.5
Q3 multi 3 A
Q3 multi 3 AB
Q4 essay 3 x
Q5 text 0 yes
ANSWERS
eli Q1=A Q2=10.5 Q3=BA
fay Q1=a Q2=9.4 Q3=B Q3=A
eli Q1=A
gil Q9=A Q1 Q2= Q2=10
""",
"""KEY
Q7 text 10 Ok
ANSWERS
""",
"""KEY
Q1 choice x A
ANSWERS
hal Q1=A
""",
], files=["main", "key", "grade"])
def _(s):
    ls = lines_of(s); out = []
    ki = ls.index("KEY"); ai = ls.index("ANSWERS")
    key = {}; order = []
    for n in range(ki + 2, ai + 1):
        w = ls[n - 1].split()
        if not w: continue
        ok = len(w) >= 4 and re.fullmatch(r"Q\d+", w[0]) and re.fullmatch(r"\d+", w[2]) and 1 <= int(w[2]) <= 100 and w[0] not in key
        if ok:
            ty, sp = w[1], w[3:]
            if ty == "choice": ok = len(sp) == 1 and re.fullmatch(r"[A-E]", sp[0])
            elif ty == "multi": ok = len(sp) == 1 and re.fullmatch(r"[A-E]{2,}", sp[0]) and len(set(sp[0])) == len(sp[0])
            elif ty == "number":
                ok = len(sp) == 2 and all(re.fullmatch(r"[+-]?\d+(\.\d+)?", x) for x in sp) and Decimal(sp[1]) >= 0
            elif ty == "text": ok = len(sp) == 1 and all(sp[0].split("|"))
            else: ok = False
        if not ok: out.append(f"key line {n}: bad question"); continue
        key[w[0]] = (w[1], int(w[2]), w[3:]); order.append(w[0])
    def grade(q, a):
        ty, pts, sp = key[q]
        if ty == "choice": return pts if a == sp[0] else 0
        if ty == "multi":
            if not re.fullmatch(r"[A-E]+", a): return 0
            got = set(a); want = set(sp[0])
            if got == want: return pts
            return max(0, pts * (len(got & want) - len(got - want)) // len(want))
        if ty == "number":
            if not re.fullmatch(r"[+-]?\d+(\.\d+)?", a): return 0
            return pts if abs(Decimal(a) - Decimal(sp[0])) <= Decimal(sp[1]) else 0
        return pts if a.lower() in [x.lower() for x in sp[0].split("|")] else 0
    studs = []; names = set()
    for n in range(ai + 2, len(ls) + 1):
        w = ls[n - 1].split()
        if not w: continue
        if w[0] in names: out.append(f"line {n}: duplicate student"); continue
        names.add(w[0]); sc = {}
        for tok in w[1:]:
            if "=" not in tok or tok.split("=", 1)[1] == "": out.append(f"line {n}: bad answer {tok}"); continue
            q, a = tok.split("=", 1)
            if q not in key: out.append(f"line {n}: unknown question {q}"); continue
            if q in sc: out.append(f"line {n}: answered twice {q}"); continue
            sc[q] = grade(q, a)
        studs.append((w[0], sc))
    if not key: return "\n".join(out + ["empty key"]) + "\n"
    if not studs: return "\n".join(out + ["no students"]) + "\n"
    mx = sum(key[q][1] for q in order)
    tot = [(nm, sum(sc.values())) for nm, sc in studs]
    for nm, t in tot: out.append(f"{nm} {t}/{mx} {t * 100 // mx}%")
    for q in order: out.append(f"{q} avg {d2(Decimal(sum(sc.get(q, 0) for _, sc in studs)) / len(studs))}")
    hi = max(t for _, t in tot)
    out.append(f"top: {', '.join(nm for nm, t in tot if t == hi)} ({hi})")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- warehouse (multi-file)
@task("warehouse", r"""
Write a warehouse stock keeper. Read commands from stdin, one per line, numbered from 1. Blank lines are ignored.

- `DAY YYYY-MM-DD` sets today's date. It must be a real date and not before the current one (`line N: date goes back`, ignored). When the date is set, every lot whose expiry is before the new date is thrown away: print `discard SKU QTY EXPIRY` for each, ordered by SKU, then expiry, then the order received.
- `RECV SKU QTY EXPIRY` receives a lot: a positive whole quantity with an expiry date. A lot already expired (expiry before today) prints `line N: expired on arrival` and is not stored. After storing a lot, waiting backorders for that SKU are filled (below).
- `ORDER ID SKU QTY` ships QTY units of SKU. Units come from lots with the earliest expiry first (ties: the lot received first). If there is not enough, ship what there is: print `ORDER ID: shipped S of Q`, and the missing units become a backorder. If everything ships, print `ORDER ID: shipped Q`. An order ID used before prints `line N: duplicate order` and is ignored.
- `STOCK` prints one line per SKU that has units: `SKU QTY in L lots` (L = lots with units left), ordered by SKU; or `stock empty`.
- Before the first DAY line, RECV and ORDER print `line N: no date` and are ignored.
- Anything else, or a command with the wrong number of words, a quantity that is not a positive whole number, or a bad date, prints `line N: bad line`.

Backorders are filled in the order they were created, whenever a RECV stores units of their SKU, using the same lot order as orders: print `BACKORDER ID: shipped X, R left` or, when nothing is left, `BACKORDER ID: done`. A backorder that gets no units prints nothing.

At the end print `shipped T units, backordered B units`, where T counts every unit shipped (orders and backorders) and B the units still waiting.
""", [
"""DAY 2026-04-01
RECV apple 10 2026-04-05
RECV apple 5 2026-04-03
RECV pear 4 2026-05-01
ORDER o1 apple 7
STOCK
ORDER o2 pear 6
RECV pear 1 2026-06-01
RECV pear 3 2026-06-02
STOCK
""",
"""RECV milk 5 2026-01-10
DAY 2026-01-05
RECV milk 2 2026-01-04
RECV milk 6 2026-01-08
RECV milk 6 2026-01-07
RECV egg 12 2026-01-07
DAY 2026-01-08
STOCK
DAY 2026-01-07
ORDER a milk 3
ORDER a milk 1
ORDER b milk 0
ORDER c egg 2
""",
"""DAY 2026-02-30
DAY 2026-02-01
ORDER x1 tea 5
ORDER x2 tea 3
ORDER x3 coffee 1
RECV tea 6 2026-03-01
RECV tea 6 2026-03-01
STOCK
SELL tea 1
RECV tea 2.5 2026-03-01
""",
"""DAY 2026-09-09
STOCK
RECV b 1 2026-09-09
RECV a 2 2026-09-10
RECV a 3 2026-09-09
DAY 2026-09-10
STOCK
""",
], files=["main", "stock", "orders"])
def _(s):
    out = []; today = None; lots = []; seq = 0; ids = set(); back = []; shipped = 0
    def take(sku, q):
        nonlocal shipped
        got = 0
        for lot in sorted((l for l in lots if l[0] == sku and l[1] > 0), key=lambda l: (l[2], l[3])):
            k = min(lot[1], q - got); lot[1] -= k; got += k
            if got == q: break
        shipped += got
        return got
    for n, raw in enumerate(lines_of(s), 1):
        w = raw.split()
        if not w: continue
        c = w[0]
        posint = lambda x: re.fullmatch(r"\d+", x) and int(x) > 0
        if c == "DAY" and len(w) == 2 and date_of(w[1]):
            d = date_of(w[1])
            if today and d < today: out.append(f"line {n}: date goes back"); continue
            today = d
            gone = sorted((l for l in lots if l[1] > 0 and l[2] < today), key=lambda l: (l[0], l[2], l[3]))
            for l in gone: out.append(f"discard {l[0]} {l[1]} {l[2]}"); l[1] = 0
        elif c == "RECV" and len(w) == 4 and posint(w[2]) and date_of(w[3]):
            if not today: out.append(f"line {n}: no date"); continue
            e = date_of(w[3])
            if e < today: out.append(f"line {n}: expired on arrival"); continue
            seq += 1; lots.append([w[1], int(w[2]), e, seq])
            for b in back:
                if b[1] != w[1] or b[2] == 0: continue
                got = take(w[1], b[2])
                if got == 0: continue
                b[2] -= got
                out.append(f"BACKORDER {b[0]}: done" if b[2] == 0 else f"BACKORDER {b[0]}: shipped {got}, {b[2]} left")
        elif c == "ORDER" and len(w) == 4 and posint(w[3]):
            if not today: out.append(f"line {n}: no date"); continue
            if w[1] in ids: out.append(f"line {n}: duplicate order"); continue
            ids.add(w[1]); q = int(w[3]); got = take(w[2], q)
            if got == q: out.append(f"ORDER {w[1]}: shipped {q}")
            else:
                out.append(f"ORDER {w[1]}: shipped {got} of {q}"); back.append([w[1], w[2], q - got])
        elif w == ["STOCK"]:
            skus = sorted({l[0] for l in lots if l[1] > 0})
            if not skus: out.append("stock empty")
            for k in skus:
                ls_ = [l for l in lots if l[0] == k and l[1] > 0]
                out.append(f"{k} {sum(l[1] for l in ls_)} in {len(ls_)} lots")
        else:
            out.append(f"line {n}: bad line")
    out.append(f"shipped {shipped} units, backordered {sum(b[2] for b in back)} units")
    return "\n".join(out) + "\n"

ROLES = {
    "quiz": "`key` reads and checks the answer key and grades one answer against a question; `grade` keeps the students' scores and works out totals, averages and the top score; `main` splits stdin into its sections, calls them and prints.",
    "warehouse": "`stock` holds the lots and picks units in expiry order; `orders` tracks order IDs and backorders; `main` reads and checks the commands, keeps the date and totals, and prints.",
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
