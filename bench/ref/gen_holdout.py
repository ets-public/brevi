# Held-out tasks: written after v0.6, never used to tune Axon. Writes tasks/<name>/{prompt.md,tests/N.in,N.out}
import os, collections, math
T = {}
def task(name, prompt, inputs):
    def deco(fn):
        T[name] = (prompt, inputs, fn); return fn
    return deco

@task("anagrams", "Stdin holds words separated by whitespace. Group words that are anagrams of each other (same letters, case-insensitive; words are compared lowercased and duplicates count once). Print one group per line: its words sorted alphabetically and joined by spaces. Order groups by size, largest first, then by their first word. Skip groups with only one word.",
      ["listen silent enlist google tinsel banana\nLISTEN act cat tac\n", "abc bca cab xyz zyx q\n", "one two\n"])
def _(s):
    g = collections.defaultdict(set)
    for w in s.split():
        w = w.lower(); g["".join(sorted(w))].add(w)
    gs = [sorted(v) for v in g.values() if len(v) > 1]
    gs.sort(key=lambda v: (-len(v), v[0]))
    return "".join(" ".join(v) + "\n" for v in gs)

@task("log_stats", "Each stdin line is a web log entry `METHOD PATH STATUS MS`, for example `GET /home 200 35`. For each path, sorted alphabetically, print `PATH COUNT AVG P90 ERR%`: COUNT is the number of requests, AVG the mean of MS with 1 decimal place, P90 the smallest MS value such that at least 90% of that path's requests took that long or less, and ERR% the percentage of requests with STATUS 500 or more, with 1 decimal place followed by `%`. Ignore blank lines.",
      ["GET /home 200 35\nGET /home 200 45\nPOST /login 500 120\nGET /home 503 300\nPOST /login 200 80\n\n",
       "\n".join(f"GET /a {200 if i % 7 else 500} {i * 3 % 50 + 1}" for i in range(1, 31)) + "\n",
       "PUT /x 201 5\n"])
def _(s):
    d = collections.defaultdict(list)
    for l in s.split("\n"):
        if not l.strip(): continue
        m, p, st, ms = l.split(); d[p].append((int(st), int(ms)))
    o = []
    for p in sorted(d):
        r = d[p]; ms = sorted(x[1] for x in r); n = len(r)
        p90 = ms[math.ceil(0.9 * n) - 1]
        err = sum(1 for x in r if x[0] >= 500) * 100 / n
        o.append(f"{p} {n} {sum(ms)/n:.1f} {p90} {err:.1f}%")
    return "\n".join(o) + "\n"

@task("spiral", "The first stdin line has two integers R and C. The next R lines each have C integers: a matrix. Print the matrix's numbers in clockwise spiral order starting at the top-left corner, all on one line separated by spaces. Then print the sum of each column on a second line, separated by spaces.",
      ["3 3\n1 2 3\n4 5 6\n7 8 9\n", "2 4\n1 2 3 4\n5 6 7 8\n", "1 1\n-5\n"])
def _(s):
    L = s.strip().split("\n"); r, c = map(int, L[0].split())
    m = [list(map(int, L[1 + i].split())) for i in range(r)]
    out = []; top, bot, lef, rig = 0, r - 1, 0, c - 1
    while top <= bot and lef <= rig:
        for j in range(lef, rig + 1): out.append(m[top][j])
        for i in range(top + 1, bot + 1): out.append(m[i][rig])
        if top < bot:
            for j in range(rig - 1, lef - 1, -1): out.append(m[bot][j])
        if lef < rig:
            for i in range(bot - 1, top, -1): out.append(m[i][lef])
        top += 1; bot -= 1; lef += 1; rig -= 1
    cols = [sum(m[i][j] for i in range(r)) for j in range(c)]
    return " ".join(map(str, out)) + "\n" + " ".join(map(str, cols)) + "\n"

@task("brackets", "For each stdin line, check whether its brackets `()[]{}` are balanced; other characters are ignored. Print `ok` if balanced. Otherwise print `error at N` where N is the 1-based character position of the first closing bracket that does not match, or, if every closing bracket matched but some are left open, `error at N` where N is the length of the line plus 1.",
      ["(a[b]{c})\n([)]\n((x)\n\n}\n", "{[()()]}\nabc\n[(])\n", "(((\n"])
def _(s):
    o = []
    for l in s.split("\n")[:-1]:
        st = []; bad = 0
        for i, ch in enumerate(l):
            if ch in "([{": st.append(ch)
            elif ch in ")]}":
                if not st or "([{"[")]}".index(ch)] != st[-1]: bad = i + 1; break
                st.pop()
        if bad: o.append(f"error at {bad}")
        elif st: o.append(f"error at {len(l) + 1}")
        else: o.append("ok")
    return "".join(x + "\n" for x in o)

@task("schedule", "Each stdin line describes a job: `NAME DURATION` or `NAME DURATION after A B ...` meaning the job can start only when all listed jobs have finished. Jobs run in parallel as soon as they can. Print each job as `NAME START END`, ordered by start time and then by name, then a last line `total T` where T is when the last job ends. If the dependencies contain a cycle or name an unknown job, print only `invalid`.",
      ["build 3 after fetch\nfetch 2\ntest 4 after build\nlint 1 after fetch\ndocs 5\n", "a 1 after b\nb 1 after a\n", "x 2\ny 3 after x z\n"])
def _(s):
    jobs = {}
    for l in s.strip().split("\n"):
        p = l.split(); deps = p[3:] if len(p) > 2 else []
        jobs[p[0]] = (int(p[1]), deps)
    if any(d not in jobs for _, ds in jobs.values() for d in ds): return "invalid\n"
    start = {}; state = {}
    def visit(n):
        if state.get(n) == 1: raise ValueError
        if state.get(n) == 2: return start[n] + jobs[n][0]
        state[n] = 1
        st = max([visit(d) for d in jobs[n][1]], default=0)
        state[n] = 2; start[n] = st
        return st + jobs[n][0]
    try:
        for n in jobs: visit(n)
    except ValueError:
        return "invalid\n"
    rows = sorted(jobs, key=lambda n: (start[n], n))
    total = max(start[n] + jobs[n][0] for n in jobs)
    return "".join(f"{n} {start[n]} {start[n] + jobs[n][0]}\n" for n in rows) + f"total {total}\n"

@task("roman_calc", "Each stdin line is an expression of Roman numerals joined by `+` or `-` with spaces around the operators, for example `XIV + IX - III`. Evaluate left to right and print the result as a Roman numeral. If the result is zero or less, print `nulla`. Numerals use I V X L C D M with standard subtractive pairs, and results stay below 4000.",
      ["XIV + IX - III\nMCMXCIV\nV - V\n", "MMM + CMXCIX\nI + I + I + I\nX - XI\n", "XL + II\n"])
def _(s):
    V = {"I": 1, "V": 5, "X": 10, "L": 50, "C": 100, "D": 500, "M": 1000}
    def rv(t):
        n = 0
        for i, ch in enumerate(t):
            v = V[ch]
            if i + 1 < len(t) and V[t[i + 1]] > v: n -= v
            else: n += v
        return n
    def tor(n):
        o = ""
        for v, sym in [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")]:
            while n >= v: o += sym; n -= v
        return o
    o = []
    for l in s.strip().split("\n"):
        p = l.split(); n = rv(p[0])
        for i in range(1, len(p), 2):
            n = n + rv(p[i + 1]) if p[i] == "+" else n - rv(p[i + 1])
        o.append(tor(n) if n > 0 else "nulla")
    return "".join(x + "\n" for x in o)

@task("ledger", "Stdin is CSV with the header `date,account,amount` (amount has up to 2 decimals and may be negative), followed by rows. Print, for each month that appears (format YYYY-MM, in order), a line `YYYY-MM IN OUT NET` where IN is the total of positive amounts, OUT the total of negative amounts as a positive number, and NET = IN - OUT, all with exactly 2 decimals. Then print the account with the largest final balance as `top ACCOUNT BALANCE` (ties: alphabetically first).",
      ["date,account,amount\n2024-01-05,cash,100.50\n2024-01-20,bank,-20.25\n2024-02-01,cash,-0.10\n2024-02-03,bank,300\n",
       "date,account,amount\n2023-12-31,a,0.10\n2023-12-31,a,0.20\n2023-12-31,b,0.30\n", "date,account,amount\n2024-03-01,z,-5\n"])
def _(s):
    from decimal import Decimal
    m = collections.OrderedDict(); bal = collections.defaultdict(Decimal)
    rows = [l.split(",") for l in s.strip().split("\n")[1:]]
    for d, a, x in sorted(rows, key=lambda r: r[0]):
        k = d[:7]; v = Decimal(x)
        i, o = m.get(k, (Decimal(0), Decimal(0)))
        m[k] = (i + v, o) if v > 0 else (i, o - v)
        bal[a] += v
    out = [f"{k} {i:.2f} {o:.2f} {i - o:.2f}" for k, (i, o) in sorted(m.items())]
    top = sorted(bal.items(), key=lambda kv: (-kv[1], kv[0]))[0]
    out.append(f"top {top[0]} {top[1]:.2f}")
    return "\n".join(out) + "\n"

@task("knight", "The first stdin line has N, the board size (N by N, squares named by column letter a.. and row number 1..N, like `a1`). Each following line has two squares `FROM TO`. For each, print the minimum number of knight moves from FROM to TO, or `-1` if it cannot be reached.",
      ["8\na1 h8\na1 a1\nb1 c3\n", "3\na1 b2\na1 c3\n", "1\na1 a1\n"])
def _(s):
    L = s.strip().split("\n"); n = int(L[0]); o = []
    for l in L[1:]:
        a, b = l.split()
        src = (ord(a[0]) - 97, int(a[1:]) - 1); dst = (ord(b[0]) - 97, int(b[1:]) - 1)
        dist = {src: 0}; q = collections.deque([src])
        while q:
            x, y = q.popleft()
            for dx, dy in [(1,2),(2,1),(-1,2),(-2,1),(1,-2),(2,-1),(-1,-2),(-2,-1)]:
                p = (x + dx, y + dy)
                if 0 <= p[0] < n and 0 <= p[1] < n and p not in dist:
                    dist[p] = dist[(x, y)] + 1; q.append(p)
        o.append(str(dist.get(dst, -1)))
    return "\n".join(o) + "\n"

for name, (prompt, inputs, fn) in T.items():
    d = f"tasks/{name}"; os.makedirs(f"{d}/tests", exist_ok=True)
    open(f"{d}/prompt.md", "w").write(prompt + "\n")
    for i, inp in enumerate(inputs):
        open(f"{d}/tests/{i}.in", "w").write(inp)
        open(f"{d}/tests/{i}.out", "w").write(fn(inp))
print(len(T), "tasks")
