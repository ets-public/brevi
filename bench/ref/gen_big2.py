# Held-out big tasks (written 2026-10-31, after Brevi v0.15; never used to tune the language).
# Writes tasks/<name>/{prompt.md,tests/N.in,N.out}
import os, re, csv, io, heapq, datetime
from decimal import Decimal
T = {}
def task(name, prompt, inputs):
    def deco(fn):
        T[name] = (prompt.strip(), inputs, fn); return fn
    return deco

# ---------------------------------------------------------------- sales_report
@task("sales_report", """
Read a CSV sales file from stdin and print a report. The first line is the header `date,region,product,qty,unit_price`. Fields may be quoted with double quotes (a quoted field can contain commas, and `""` inside quotes is one quote character). Ignore blank lines.

A data row is valid when it has exactly 5 fields, `date` is `YYYY-MM-DD`, `qty` is a whole number (digits only, at least 1 digit) and `unit_price` is digits with an optional `.` and 1 or 2 digits. Revenue of a row is `qty * unit_price`, computed exactly.

Print, in this order:
- `rows N` (valid rows) and then `skipped M` (invalid data rows).
- `regions`, then one line per region: two spaces, the region, a space and its total revenue with exactly 2 decimals. Order by revenue, highest first; ties by region name.
- `top_product NAME QTY`: the product with the largest total qty (ties: alphabetically first name).
- `months`, then one line per month `  YYYY-MM REVENUE` (2 decimals), in month order.

If there are no valid rows, print only the `rows` and `skipped` lines.
""", [
"""date,region,product,qty,unit_price
2026-01-05,North,Widget,3,2.50
2026-01-17,South,Gadget,1,10
2026-02-01,North,"Widget, large",2,4.25
2026-02-11,East,Gadget,5,9.99
bad,row
2026-02-12,East,Gizmo,x,1.00
2026-03-01,South,Widget,10,2.5

2026-03-02,"Sou""th",Gizmo,1,0.05
""",
"""date,region,product,qty,unit_price
2026-05-01,A,P,1,1.005
2026-05-02,A,P,2,
""",
"""date,region,product,qty,unit_price
2025-12-31,West,Pen,4,1.10
2026-01-01,East,Ink,4,1.10
2026-01-01,West,Ink,0,3.00
2026-01-02,East,Pen,2,0.75
2025-12-30,"North, Far",Pad,1,99.99
""",
])
def _(s):
    lines = [l for l in s.split("\n") if l.strip()]
    rows = list(csv.reader(io.StringIO("\n".join(lines[1:]))))
    good = []; bad = 0
    for r in rows:
        if len(r) != 5 or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", r[0]) or not re.fullmatch(r"\d+", r[3]) or not re.fullmatch(r"\d+(\.\d{1,2})?", r[4]):
            bad += 1; continue
        good.append((r[0], r[1], r[2], int(r[3]), Decimal(r[4])))
    out = [f"rows {len(good)}", f"skipped {bad}"]
    if not good: return "\n".join(out) + "\n"
    reg = {}; pq = {}; mon = {}
    for d, rg, p, q, u in good:
        v = q * u
        reg[rg] = reg.get(rg, Decimal(0)) + v
        pq[p] = pq.get(p, 0) + q
        mon[d[:7]] = mon.get(d[:7], Decimal(0)) + v
    out.append("regions")
    for rg, v in sorted(reg.items(), key=lambda x: (-x[1], x[0])): out.append(f"  {rg} {v:.2f}")
    p, q = sorted(pq.items(), key=lambda x: (-x[1], x[0]))[0]
    out.append(f"top_product {p} {q}")
    out.append("months")
    for m in sorted(mon): out.append(f"  {m} {mon[m]:.2f}")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- route_planner
@task("route_planner", """
Find shortest train routes. Stdin first has edge lines `A B MINUTES` (an undirected track between stations A and B; names have no spaces; MINUTES is a positive integer; a repeated pair keeps the smaller time). A blank line ends the edges. Every later non-blank line is a query `FROM TO`.

For each query print one line:
- If FROM or TO is not a station in any edge: `unknown station NAME` (check FROM first).
- If FROM equals TO: `FROM (0 min)`.
- If TO cannot be reached: `no route FROM TO`.
- Otherwise the stations of the fastest route joined by ` -> `, then ` (N min)`. If several routes are equally fast, take the one with the fewest stations; if still tied, the one whose list of station names is smallest when compared name by name.
""", [
"""A B 5
B C 5
A C 10
C D 1
B D 7
X Y 2

A D
D A
A A
A Y
Q A
A Q
B C
""",
"""Oslo Bergen 400
Oslo Lillehammer 120
Lillehammer Trondheim 250
Bergen Trondheim 600
Oslo Trondheim 380
Oslo Bergen 390

Oslo Trondheim
Bergen Lillehammer
Trondheim Oslo
""",
"""a b 1
b c 1
a d 1
d c 1
c e 1

a e
e a
c c
""",
])
def _(s):
    parts = s.split("\n")
    i = 0; g = {}
    while i < len(parts) and parts[i].strip():
        a, b, m = parts[i].split(); m = int(m)
        for x, y in ((a, b), (b, a)):
            g.setdefault(x, {})
            g[x][y] = min(g[x].get(y, m), m)
        i += 1
    out = []
    for l in parts[i:]:
        if not l.strip(): continue
        f, t = l.split()
        if f not in g: out.append(f"unknown station {f}"); continue
        if t not in g: out.append(f"unknown station {t}"); continue
        if f == t: out.append(f"{f} (0 min)"); continue
        best = {}
        pq = [(0, 1, [f])]
        res = None
        while pq:
            d, n, path = heapq.heappop(pq)
            u = path[-1]
            if u in best: continue
            best[u] = (d, n, path)
            if u == t: res = (d, path); break
            for v, w in g[u].items():
                if v not in best: heapq.heappush(pq, (d + w, n + 1, path + [v]))
        out.append(f"no route {f} {t}" if res is None else " -> ".join(res[1]) + f" ({res[0]} min)")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- ini_config
@task("ini_config", """
Read an INI file, then answer queries. Stdin has the INI text, then a line `---`, then one query per line.

INI rules: a line `[name]` starts a section (trim spaces inside the brackets). `key = value` sets a key (trim spaces around key and value; split at the first `=`). Lines starting with `;` or `#` (after leading spaces) and blank lines are ignored. Keys before any section belong to section `default`. A key set again in the same section replaces the earlier value. Section and key names are case-sensitive.

Values may contain references `${key}` (looked up in the same section, then in `default`) or `${section.key}`. References are replaced recursively. A reference to a missing key becomes the empty string. If resolving a value would need a key that is already being resolved (a cycle), the whole queried value is `<cycle>`.

Queries (ignore blank lines):
- `section.key`: print the resolved value, or `<missing>` if the key does not exist.
- `section.*`: print `key=value` (resolved) for every key of the section, sorted by key, or `<missing>` if there is no such section.
- `sections`: print all section names that have at least one key, sorted, separated by single spaces.
""", [
"""; global
name = demo
root = /srv/${name}

[paths]
logs = ${root}/logs
cache = ${default.root}/cache
tmp = ${missing}/tmp
 [ db ]
host = localhost
url = postgres://${host}:${port}/${name}
port = 5432
port = 6543
# loop
a = ${b}
b = ${a}
c = x${d}
d = ok
---
paths.logs
paths.cache
paths.tmp
db.url
db.a
db.c
db.zz
db.*
nope.*
sections
default.root
""",
"""[s]
k = v = w
e =
self = ${self}
[t]
ref = ${s.k}!${s.e}!
---
s.k
s.e
s.self
t.ref
t.*
sections
""",
])
def _(s):
    ini, _, qs = s.partition("\n---\n")
    sec = "default"; data = {}
    for l in ini.split("\n"):
        t = l.strip()
        if not t or t[0] in ";#": continue
        if t.startswith("[") and t.endswith("]"):
            sec = t[1:-1].strip(); continue
        if "=" in t:
            k, v = t.split("=", 1)
            data.setdefault(sec, {})[k.strip()] = v.strip()
    class Cycle(Exception): pass
    def look(sc, ref):
        if "." in ref:
            a, b = ref.split(".", 1)
            return (a, b) if b in data.get(a, {}) else None
        if ref in data.get(sc, {}): return (sc, ref)
        if ref in data.get("default", {}): return ("default", ref)
        return None
    def resolve(sc, k, busy):
        if (sc, k) in busy: raise Cycle()
        busy = busy | {(sc, k)}
        def rep(m):
            r = look(sc, m.group(1))
            return "" if r is None else resolve(r[0], r[1], busy)
        return re.sub(r"\$\{([^}]*)\}", rep, data[sc][k])
    def val(sc, k):
        try: return resolve(sc, k, frozenset())
        except Cycle: return "<cycle>"
    out = []
    for q in qs.split("\n"):
        q = q.strip()
        if not q: continue
        if q == "sections": out.append(" ".join(sorted(x for x in data if data[x]))); continue
        sc, k = q.split(".", 1)
        if k == "*":
            if sc not in data: out.append("<missing>")
            else: out += [f"{kk}={val(sc, kk)}" for kk in sorted(data[sc])]
        elif k in data.get(sc, {}): out.append(val(sc, k))
        else: out.append("<missing>")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- stack_vm
@task("stack_vm", """
Run a program for a small stack machine read from stdin, one instruction per line. `#` starts a comment to the end of the line; blank lines are ignored. A line `name:` defines a label at the next instruction. Integers are 64-bit.

Instructions: `push N` (N may be negative), `pop`, `dup`, `swap` (swap the top two), `add`, `sub`, `mul`, `div`, `mod` (for these the top value is the right operand: `push 7`, `push 2`, `sub` gives 5; `div` and `mod` round toward zero), `jmp L`, `jz L` (pop a value and jump if it is 0), `jnz L` (pop and jump if not 0), `call L` (push the return position on a separate call stack and jump), `ret` (return; with an empty call stack, stop), `print` (pop and print the value), `halt`.

Before running, check the program: the first line with an unknown instruction gives `error line N: unknown instruction X`; otherwise the first jump or call to an undefined label gives `error line N: unknown label L`. N is the line number in the input (1-based). In that case run nothing.

While running, stop with `error line N: stack underflow` when an instruction needs more values than the stack has, `error line N: division by zero` for `div` or `mod` by 0, or `error: step limit` once 10000 instructions have run without stopping. Output printed before an error stays. The program stops normally after its last instruction or at `halt`.
""", [
"""# countdown
push 3
loop:
dup
print
push 1
sub
dup
jnz loop
pop
push 7
push 2
sub
print
push -7
push 2
div
print
push -7
push 2
mod
print
""",
"""push 5
call square
print
halt
square:   # n -> n*n
dup
mul
ret
""",
"""push 1
print
add
""",
"""push 1
jmp nowhere
""",
"""push 1
frob 2
jmp nowhere
""",
"""top:
jmp top
""",
"""push 4
push 0
div
""",
"""push 2
push 3
swap
print
print
ret
print
""",
])
def _(s):
    prog = []; labels = {}
    lines = s.split("\n")
    ops = {"push": 1, "pop": 0, "dup": 0, "swap": 0, "add": 0, "sub": 0, "mul": 0, "div": 0, "mod": 0, "jmp": 1, "jz": 1, "jnz": 1, "call": 1, "ret": 0, "print": 0, "halt": 0}
    for n, l in enumerate(lines, 1):
        t = l.split("#", 1)[0].strip()
        if not t: continue
        if t.endswith(":") and len(t.split()) == 1: labels[t[:-1]] = len(prog); continue
        p = t.split()
        if p[0] not in ops: return f"error line {n}: unknown instruction {p[0]}\n"
        prog.append((n, p[0], p[1] if len(p) > 1 else None))
    for n, op, a in prog:
        if op in ("jmp", "jz", "jnz", "call") and a not in labels: return f"error line {n}: unknown label {a}\n"
    out = []; st = []; cs = []; pc = 0; steps = 0
    def done(msg=None):
        return "\n".join(out + ([msg] if msg else [])) + ("\n" if out or msg else "")
    need = {"pop": 1, "dup": 1, "swap": 2, "add": 2, "sub": 2, "mul": 2, "div": 2, "mod": 2, "jz": 1, "jnz": 1, "print": 1}
    while pc < len(prog):
        if steps >= 10000: return done("error: step limit")
        steps += 1
        n, op, a = prog[pc]; pc += 1
        if len(st) < need.get(op, 0): return done(f"error line {n}: stack underflow")
        if op == "push": st.append(int(a))
        elif op == "pop": st.pop()
        elif op == "dup": st.append(st[-1])
        elif op == "swap": st[-1], st[-2] = st[-2], st[-1]
        elif op in ("add", "sub", "mul", "div", "mod"):
            b = st.pop(); x = st.pop()
            if op in ("div", "mod") and b == 0: return done(f"error line {n}: division by zero")
            if op == "add": st.append(x + b)
            elif op == "sub": st.append(x - b)
            elif op == "mul": st.append(x * b)
            else:
                q = abs(x) // abs(b) * (1 if (x < 0) == (b < 0) else -1)
                st.append(q if op == "div" else x - q * b)
        elif op == "jmp": pc = labels[a]
        elif op == "jz":
            if st.pop() == 0: pc = labels[a]
        elif op == "jnz":
            if st.pop() != 0: pc = labels[a]
        elif op == "call": cs.append(pc); pc = labels[a]
        elif op == "ret":
            if not cs: break
            pc = cs.pop()
        elif op == "print": out.append(str(st.pop()))
        elif op == "halt": break
    return done()

# ---------------------------------------------------------------- line_diff
@task("line_diff", """
Print a line diff of two texts. Stdin holds the old text, then a line that is exactly `=====`, then the new text. Split each text into lines (a final newline does not make an extra empty line; an empty text has no lines).

Use a longest common subsequence of lines. Walk both texts from the start with positions i (old) and j (new):
- if old[i] equals new[j], print `  ` + the line and move both on;
- otherwise, if the LCS length of (old from i+1, new from j) is at least that of (old from i, new from j+1), print `- ` + old[i] and move i on;
- otherwise print `+ ` + new[j] and move j on.
When one text runs out, print the rest of the other with `- ` or `+ `.

Then print `@@ -A +B @@` where A is the number of removed lines and B the number of added lines. If the texts are identical, print only `no changes`.
""", [
"""a
b
c
d
=====
a
c
d
e
""",
"""same
lines
=====
same
lines
""",
"""=====
new
""",
"""x
y
z
=====
z
y
x
""",
"""fn main() {
    let a = 1;
    let b = 2;
    print(a + b);
}
=====
fn main() {
    let a = 1;
    let c = 3;
    let b = 2;
    print(a + b + c);
}
""",
])
def _(s):
    old, _, new = s.partition("=====\n")
    a = old.split("\n")[:-1] if old else []
    if old and not old.endswith("\n"): a = old.split("\n")
    b = new.split("\n")
    if b and b[-1] == "": b = b[:-1]
    n, m = len(a), len(b)
    L = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(n - 1, -1, -1):
        for j in range(m - 1, -1, -1):
            L[i][j] = L[i + 1][j + 1] + 1 if a[i] == b[j] else max(L[i + 1][j], L[i][j + 1])
    if a == b: return "no changes\n"
    out = []; i = j = 0; rm = ad = 0
    while i < n and j < m:
        if a[i] == b[j]: out.append("  " + a[i]); i += 1; j += 1
        elif L[i + 1][j] >= L[i][j + 1]: out.append("- " + a[i]); i += 1; rm += 1
        else: out.append("+ " + b[j]); j += 1; ad += 1
    while i < n: out.append("- " + a[i]); i += 1; rm += 1
    while j < m: out.append("+ " + b[j]); j += 1; ad += 1
    out.append(f"@@ -{rm} +{ad} @@")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- room_booking
@task("room_booking", """
Book meeting rooms. Each stdin line is a command (ignore blank lines):

- `ROOM name capacity`: add a room.
- `BOOK id date start end people`: date is `YYYY-MM-DD`, start and end are `HH:MM` (end after start). Pick the room with the smallest capacity that is at least `people` and has no booking on that date overlapping the time (a booking ending exactly when another starts does not overlap). Ties: room name, alphabetically. Print `id -> room`, or `id rejected` if no room fits. Booking ids are unique.
- `CANCEL id`: print `id cancelled`, or `id not found` if there is no such active booking.
- `LIST date`: print `date Weekday` (the English weekday name, such as `Monday`), then one line per booking on that date: `  HH:MM-HH:MM room id`, sorted by start time, then room name. If there are none, print `  no bookings` after the header.
""", [
"""ROOM Big 10
ROOM Small 4
ROOM Tiny 2
ROOM Mid 4
BOOK b1 2026-11-02 09:00 10:00 3
BOOK b2 2026-11-02 09:30 10:30 3
BOOK b3 2026-11-02 09:15 09:45 4
BOOK b4 2026-11-02 10:00 11:00 4
BOOK b5 2026-11-02 08:00 12:00 11
BOOK b6 2026-11-02 09:00 09:30 1
LIST 2026-11-02
CANCEL b2
CANCEL b2
BOOK b7 2026-11-02 09:40 09:50 2
LIST 2026-11-03
LIST 2026-11-02
""",
"""ROOM A 5
BOOK x 2024-02-29 13:00 14:00 5
BOOK y 2024-02-29 14:00 15:00 5
BOOK z 2024-02-29 14:59 15:01 5
BOOK w 2024-03-01 14:59 15:01 5
LIST 2024-02-29
LIST 2024-03-01
""",
])
def _(s):
    rooms = {}; book = {}; out = []
    mins = lambda t: int(t[:2]) * 60 + int(t[3:])
    for l in s.split("\n"):
        p = l.split()
        if not p: continue
        if p[0] == "ROOM": rooms[p[1]] = int(p[2])
        elif p[0] == "BOOK":
            bid, d, a, b, n = p[1], p[2], mins(p[3]), mins(p[4]), int(p[5])
            ok = [r for r in rooms if rooms[r] >= n and not any(x[0] == r and x[1] == d and x[2] < b and a < x[3] for x in book.values())]
            if not ok: out.append(f"{bid} rejected"); continue
            r = min(ok, key=lambda r: (rooms[r], r))
            book[bid] = (r, d, a, b, p[3], p[4]); out.append(f"{bid} -> {r}")
        elif p[0] == "CANCEL":
            out.append(f"{p[1]} cancelled" if book.pop(p[1], None) else f"{p[1]} not found")
        elif p[0] == "LIST":
            d = p[1]
            out.append(f"{d} {datetime.date.fromisoformat(d).strftime('%A')}")
            bs = sorted((x[2], x[0], k, x) for k, x in book.items() if x[1] == d)
            if not bs: out.append("  no bookings")
            for _, r, k, x in bs: out.append(f"  {x[4]}-{x[5]} {r} {k}")
    return "\n".join(out) + "\n"

for name, (prompt, inputs, fn) in T.items():
    d = f"tasks/{name}"; os.makedirs(f"{d}/tests", exist_ok=True)
    open(f"{d}/prompt.md", "w").write(prompt + "\n")
    for i, inp in enumerate(inputs):
        open(f"{d}/tests/{i}.in", "w").write(inp)
        open(f"{d}/tests/{i}.out", "w").write(fn(inp))
print(len(T), "tasks")
