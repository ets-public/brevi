# Reference solutions and test inputs; writes tasks/<name>/{prompt.md,tests/N.in,N.out}
import os, collections, statistics
T = {}
def task(name, prompt, inputs):
    def deco(fn):
        T[name] = (prompt, inputs, fn); return fn
    return deco

@task("sum_numbers", "Read integers from stdin, one per line (blank lines may appear and should be ignored). Print their sum.",
      ["1\n2\n3\n", "10\n\n-4\n", "0\n"])
def _(s): return str(sum(int(l) for l in s.split("\n") if l.strip())) + "\n"

@task("word_freq", "Read text from stdin. Words are separated by whitespace; compare them lowercased. Print the 3 most frequent words as `word count`, one per line, most frequent first; break ties alphabetically. If fewer than 3 distinct words, print them all.",
      ["the cat and the dog and THE bird\n", "b a b a c\n", "solo\n"])
def _(s):
    c = collections.Counter(w.lower() for w in s.split())
    items = sorted(c.items(), key=lambda kv: (-kv[1], kv[0]))[:3]
    return "".join(f"{w} {n}\n" for w, n in items)

@task("fizzbuzz", "Read an integer n from stdin. For i from 1 to n print `FizzBuzz` if i is divisible by 15, `Fizz` if by 3, `Buzz` if by 5, otherwise i. One per line.",
      ["5\n", "15\n", "1\n"])
def _(s):
    n = int(s); o = []
    for i in range(1, n + 1):
        o.append("FizzBuzz" if i % 15 == 0 else "Fizz" if i % 3 == 0 else "Buzz" if i % 5 == 0 else str(i))
    return "\n".join(o) + "\n"

@task("reverse_words", "For each line of stdin, print the words of that line in reverse order separated by single spaces. Empty lines stay empty.",
      ["hello world\na b c\n", "one\n\ntwo  three\n"])
def _(s):
    return "".join(" ".join(reversed(l.split())) + "\n" for l in s.split("\n")[:-1])

@task("csv_avg", "Stdin is CSV with header `name,dept,salary` followed by rows. For each dept, print `dept average` where average is the mean salary with exactly 2 decimal places. Sort by dept name.",
      ["name,dept,salary\nann,eng,100\nbob,eng,51\ncat,ops,70\n", "name,dept,salary\nz,b,1\ny,a,2\nx,a,3\n"])
def _(s):
    d = collections.defaultdict(list)
    for l in s.strip().split("\n")[1:]:
        n, dep, sal = l.split(","); d[dep].append(int(sal))
    return "".join(f"{k} {sum(v)/len(v):.2f}\n" for k, v in sorted(d.items()))

@task("bank", "Process commands from stdin, one per line: `deposit NAME AMOUNT`, `withdraw NAME AMOUNT`, `balance NAME`. Accounts start at 0. For `balance` print `NAME BALANCE`. If a withdrawal exceeds the balance, do not change it and print `error: insufficient funds for NAME`.",
      ["deposit ann 50\nwithdraw ann 20\nbalance ann\nwithdraw ann 100\nbalance bob\n", "deposit x 1\ndeposit x 2\nbalance x\n"])
def _(s):
    b = collections.defaultdict(int); o = []
    for l in s.strip().split("\n"):
        p = l.split()
        if p[0] == "deposit": b[p[1]] += int(p[2])
        elif p[0] == "withdraw":
            if int(p[2]) > b[p[1]]: o.append(f"error: insufficient funds for {p[1]}")
            else: b[p[1]] -= int(p[2])
        else: o.append(f"{p[1]} {b[p[1]]}")
    return "".join(x + "\n" for x in o)

@task("brackets", "For each line of stdin, print `yes` if the brackets ()[]{} in it are balanced and properly nested, else `no`. Other characters are ignored.",
      ["([]{})\n(]\n((\nabc\n", "{[()()]}x\n)(\n"])
def _(s):
    o = []
    for l in s.split("\n")[:-1]:
        st = []; ok = True
        for ch in l:
            if ch in "([{": st.append(ch)
            elif ch in ")]}":
                if not st or "([{"[")]}".index(ch)] != st.pop(): ok = False; break
        o.append("yes" if ok and not st else "no")
    return "".join(x + "\n" for x in o)

@task("roman", "For each line of stdin containing an integer from 1 to 3999, print its Roman numeral.",
      ["1\n4\n9\n14\n", "3999\n444\n2024\n"])
def _(s):
    vals = [(1000,"M"),(900,"CM"),(500,"D"),(400,"CD"),(100,"C"),(90,"XC"),(50,"L"),(40,"XL"),(10,"X"),(9,"IX"),(5,"V"),(4,"IV"),(1,"I")]
    o = []
    for l in s.split():
        n = int(l); r = ""
        for v, t in vals:
            while n >= v: r += t; n -= v
        o.append(r)
    return "".join(x + "\n" for x in o)

@task("grid_path", "Stdin is a grid of characters: `.` open, `#` wall, `S` start, `E` end. Moving up, down, left or right one step at a time through non-wall cells, print the fewest steps from S to E, or -1 if unreachable.",
      ["S.#\n..#\n#.E\n", "S#E\n", "S..\n.#.\n..E\n"])
def _(s):
    g = [l for l in s.split("\n") if l]
    for r, row in enumerate(g):
        for c, ch in enumerate(row):
            if ch == "S": st = (r, c)
    q = collections.deque([(st, 0)]); seen = {st}
    while q:
        (r, c), d = q.popleft()
        if g[r][c] == "E": return f"{d}\n"
        for dr, dc in ((1,0),(-1,0),(0,1),(0,-1)):
            nr, nc = r+dr, c+dc
            if 0 <= nr < len(g) and 0 <= nc < len(g[nr]) and g[nr][nc] != "#" and (nr,nc) not in seen:
                seen.add((nr,nc)); q.append(((nr,nc), d+1))
    return "-1\n"

@task("anagrams", "Read words from stdin (whitespace separated). Group words that are anagrams of each other. Within a group sort words alphabetically and join with spaces; print one group per line, groups sorted by their first word. Duplicate words appear once.",
      ["eat tea tan ate nat bat\n", "abc\n", "listen silent enlist google\n"])
def _(s):
    g = collections.defaultdict(set)
    for w in s.split(): g["".join(sorted(w))].add(w)
    groups = sorted(sorted(v) for v in g.values())
    return "".join(" ".join(x) + "\n" for x in groups)

@task("rle", "Each stdin line is `e TEXT` or `d CODE`. For `e`, print the run-length encoding of TEXT as char then count for each run (e.g. `aaab` -> `a3b1`). For `d`, decode such a code back (counts may have several digits).",
      ["e aaab\nd a3b1\n", "e abc\nd x12y1\n"])
def _(s):
    o = []
    for l in s.strip().split("\n"):
        m, t = l.split(" ", 1)
        if m == "e":
            r = ""; i = 0
            while i < len(t):
                j = i
                while j < len(t) and t[j] == t[i]: j += 1
                r += t[i] + str(j - i); i = j
            o.append(r)
        else:
            r = ""; i = 0
            while i < len(t):
                ch = t[i]; i += 1; n = ""
                while i < len(t) and t[i].isdigit(): n += t[i]; i += 1
                r += ch * int(n)
            o.append(r)
    return "".join(x + "\n" for x in o)

@task("grades", "Each stdin line is `NAME SCORE` (integer score). Print the median score with exactly 1 decimal place on the first line, then the names with the highest score sorted alphabetically, one per line.",
      ["ann 90\nbob 70\ncat 90\n", "a 1\nb 2\nc 3\nd 4\n"])
def _(s):
    rows = [l.split() for l in s.strip().split("\n")]
    sc = [int(r[1]) for r in rows]; mx = max(sc)
    return f"{statistics.median(sc):.1f}\n" + "".join(n + "\n" for n in sorted(r[0] for r in rows if int(r[1]) == mx))

for name, (prompt, inputs, fn) in T.items():
    d = f"tasks/{name}"; os.makedirs(f"{d}/tests", exist_ok=True)
    open(f"{d}/prompt.md", "w").write(prompt + "\n")
    for i, inp in enumerate(inputs):
        open(f"{d}/tests/{i}.in", "w").write(inp)
        open(f"{d}/tests/{i}.out", "w").write(fn(inp))
print(len(T), "tasks")
