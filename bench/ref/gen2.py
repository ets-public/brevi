# Harder tasks (round 3). Writes tasks/<name>/ like gen.py.
import os, collections, heapq, json, datetime
T = {}
def task(name, prompt, inputs):
    def deco(fn):
        T[name] = (prompt, inputs, fn); return fn
    return deco

@task("calc", "Each stdin line is an integer arithmetic expression with + - * / and parentheses, possibly with spaces and unary minus (e.g. `-(2+3)*4`). `/` is integer division truncating toward zero. `*` and `/` bind tighter than `+` and `-`; operators of equal precedence are left-associative. Print each result on its own line, or `error` if a division by zero happens.",
      ["1 + 2 * 3\n(1+2)*3\n10 - 4 - 3\n", "-(2+3)*4\n7 / -2\n8 / (3 - 3)\n", "2*-3\n100/7/2\n-  -4\n"])
def _(s):
    out = []
    for line in s.strip("\n").split("\n"):
        toks = [c for c in line if not c.isspace()]
        pos = [0]
        def peek(): return toks[pos[0]] if pos[0] < len(toks) else None
        def take(): pos[0] += 1; return toks[pos[0]-1]
        def expr():
            v = term()
            while peek() in ("+", "-"):
                op = take(); r = term(); v = v + r if op == "+" else v - r
            return v
        def term():
            v = unary()
            while peek() in ("*", "/"):
                op = take(); r = unary()
                if op == "*": v *= r
                else:
                    if r == 0: raise ZeroDivisionError
                    q = abs(v) // abs(r); v = q if (v >= 0) == (r >= 0) else -q
            return v
        def unary():
            if peek() == "-": take(); return -unary()
            if peek() == "(":
                take(); v = expr(); take(); return v
            n = ""
            while peek() is not None and peek().isdigit(): n += take()
            return int(n)
        try: out.append(str(expr()))
        except ZeroDivisionError: out.append("error")
    return "".join(x + "\n" for x in out)

@task("topo", "Each stdin line is `A -> B`, meaning task A must come before task B. Print all tasks in an order that respects every rule, one per line. Whenever several tasks are free to go next, pick the alphabetically smallest. If the rules contain a cycle, print only `cycle`.",
      ["b -> c\na -> c\n", "x -> y\ny -> z\nz -> x\n", "d -> a\nc -> a\nb -> a\ne -> d\n"])
def _(s):
    nodes = set(); indeg = collections.Counter(); adj = collections.defaultdict(list)
    for l in s.strip().split("\n"):
        a, b = [x.strip() for x in l.split("->")]
        nodes |= {a, b}; adj[a].append(b); indeg[b] += 1
    h = [n for n in nodes if indeg[n] == 0]; heapq.heapify(h); out = []
    while h:
        n = heapq.heappop(h); out.append(n)
        for m in adj[n]:
            indeg[m] -= 1
            if indeg[m] == 0: heapq.heappush(h, m)
    if len(out) < len(nodes): return "cycle\n"
    return "".join(x + "\n" for x in out)

@task("intervals", "Each stdin line is `START END` (integers, START <= END). Merge intervals that overlap or touch (e.g. 1 3 and 3 5 merge to 1 5). Print the merged intervals sorted by start as `START END`, one per line, then a final line `total N` where N is the sum of (END - START) over merged intervals.",
      ["1 3\n2 6\n8 10\n15 18\n", "5 7\n1 3\n3 5\n", "4 4\n-2 1\n"])
def _(s):
    iv = sorted(tuple(map(int, l.split())) for l in s.strip().split("\n")); m = []
    for a, b in iv:
        if m and a <= m[-1][1]: m[-1][1] = max(m[-1][1], b)
        else: m.append([a, b])
    return "".join(f"{a} {b}\n" for a, b in m) + f"total {sum(b-a for a,b in m)}\n"

@task("lru", "First stdin line is the cache capacity N (N >= 1). Each following line is `put KEY VALUE` or `get KEY`. Simulate a least-recently-used cache: `get` prints the value or -1 if absent, and counts as a use. `put` inserts or updates the key (an update also counts as a use); if inserting a new key would exceed capacity, first evict the least recently used key.",
      ["2\nput a 1\nput b 2\nget a\nput c 3\nget b\nget a\nget c\n", "1\nput x 5\nput x 6\nget x\nput y 1\nget x\nget y\n"])
def _(s):
    lines = s.strip().split("\n"); cap = int(lines[0]); c = collections.OrderedDict(); out = []
    for l in lines[1:]:
        p = l.split()
        if p[0] == "get":
            if p[1] in c: c.move_to_end(p[1]); out.append(c[p[1]])
            else: out.append("-1")
        else:
            if p[1] in c: c.move_to_end(p[1])
            elif len(c) >= cap: c.popitem(last=False)
            c[p[1]] = p[2]
    return "".join(x + "\n" for x in out)

@task("json_sum", "Stdin holds one JSON document (objects, arrays, integers that may be negative, strings that may contain escaped quotes, true, false, null). Print the sum of all numbers in it, but ignore any object (and everything inside it) that has any property whose value is the string \"red\". Arrays containing \"red\" are not ignored. Do not use a JSON library if your language lacks one in its standard library.",
      ['[1,{"c":"red","b":2},3]\n', '{"d":"red","e":[1,2,3,4],"f":5}\n', '[1,"red",5]\n', '{"a":[-1,{"a":1}],"b":"say \\"red\\"","c":{"x":[10,null,true]}}\n'])
def _(s):
    def walk(v):
        if isinstance(v, bool) or v is None or isinstance(v, str): return 0
        if isinstance(v, int): return v
        if isinstance(v, list): return sum(walk(x) for x in v)
        if "red" in v.values(): return 0
        return sum(walk(x) for x in v.values())
    return f"{walk(json.loads(s))}\n"

@task("bigadd", "Each stdin line is two non-negative integers `A B`, each up to 60 digits long. Print A + B exactly, with no leading zeros.",
      ["1 2\n999 1\n", "123456789012345678901234567890 987654321098765432109876543210\n", "0 0\n000123 7\n"])
def _(s):
    return "".join(f"{int(a)+int(b)}\n" for a, b in (l.split() for l in s.strip().split("\n")))

@task("wrap", "The first stdin line is a width W. The rest is text. Paragraphs are separated by one or more blank lines. For each paragraph, join its words with single spaces and wrap greedily so no line exceeds W characters (a word longer than W goes on its own line). Print the wrapped paragraphs separated by exactly one blank line.",
      ["10\nthe quick brown fox jumps over the lazy dog\n", "6\naa bb cc\n\n\nsupercalifragilistic a\n", "20\none\ntwo three\n\nfour\n"])
def _(s):
    lines = s.split("\n"); w = int(lines[0]); paras = []; cur = []
    for l in lines[1:]:
        if l.strip(): cur += l.split()
        elif cur: paras.append(cur); cur = []
    if cur: paras.append(cur)
    out = []
    for p in paras:
        rows = []; line = ""
        for word in p:
            if not line: line = word
            elif len(line) + 1 + len(word) <= w: line += " " + word
            else: rows.append(line); line = word
        rows.append(line); out.append("\n".join(rows))
    return "\n\n".join(out) + "\n"

@task("inventory", "Stdin is CSV with header `item,qty,price`. The item field may be wrapped in double quotes and may then contain commas. qty is an integer; price has at most 2 decimals. For each distinct item (items can repeat; sum their values), compute total value qty*price. Print `ITEM: VALUE` with VALUE to exactly 2 decimals, sorted by value descending, ties by item name ascending. Item names are printed without quotes.",
      ['item,qty,price\napple,3,0.50\nbanana,12,0.25\napple,1,0.50\n', 'item,qty,price\n"nuts, mixed",2,3.10\n"bolts",10,0.31\nwasher,100,0.03\n'])
def _(s):
    import csv, io
    from decimal import Decimal
    tot = collections.defaultdict(Decimal)
    for r in csv.DictReader(io.StringIO(s)):
        tot[r["item"]] += int(r["qty"]) * Decimal(r["price"])
    return "".join(f"{k}: {v:.2f}\n" for k, v in sorted(tot.items(), key=lambda kv: (-kv[1], kv[0])))

@task("dijkstra", "Stdin lines are undirected weighted edges `A B W` (W a positive integer), then a final line `FROM TO`. Print the shortest distance, then on the next line the path as nodes joined by `->`. If several shortest paths exist, print the one whose node list is lexicographically smallest when compared node by node. If TO is unreachable print only `none`.",
      ["a b 1\nb c 2\na c 5\na c\n", "a b 1\nc d 1\na d\n", "s a 1\ns b 1\na t 1\nb t 1\ns t\n"])
def _(s):
    lines = s.strip().split("\n"); src, dst = lines[-1].split(); g = collections.defaultdict(list)
    for l in lines[:-1]:
        a, b, w = l.split(); g[a].append((b, int(w))); g[b].append((a, int(w)))
    best = {src: (0, [src])}; pq = [(0, [src])]
    while pq:
        d, path = heapq.heappop(pq); n = path[-1]
        if best.get(n) != (d, path): continue
        for m, w in g[n]:
            cand = (d + w, path + [m])
            if m not in best or cand < best[m]: best[m] = cand; heapq.heappush(pq, cand)
    if dst not in best: return "none\n"
    return f"{best[dst][0]}\n{'->'.join(best[dst][1])}\n"

@task("date_diff", "Each stdin line is two dates `YYYY-MM-DD YYYY-MM-DD` (Gregorian calendar, years 1 to 9999). Print the absolute number of days between them.",
      ["2024-01-01 2024-03-01\n2023-01-01 2023-03-01\n", "2000-02-28 1900-02-28\n2024-12-31 2024-12-31\n", "0001-01-01 9999-12-31\n"])
def _(s):
    p = lambda x: datetime.date(*map(int, x.split("-")))
    return "".join(f"{abs((p(b) - p(a)).days)}\n" for a, b in (l.split() for l in s.strip().split("\n")))

for name, (prompt, inputs, fn) in T.items():
    d = f"tasks/{name}"; os.makedirs(f"{d}/tests", exist_ok=True)
    open(f"{d}/prompt.md", "w").write(prompt + "\n")
    for i, inp in enumerate(inputs):
        open(f"{d}/tests/{i}.in", "w").write(inp)
        open(f"{d}/tests/{i}.out", "w").write(fn(inp))
print(len(T), "tasks")
