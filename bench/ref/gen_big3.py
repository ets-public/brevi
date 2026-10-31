# Held-out XL tasks (written 2026-10-31, after Brevi v0.16; never used to tune the language).
# Bigger than gen_big2.py, and two of them must be split over several files.
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

# ---------------------------------------------------------------- order_book
@task("order_book", """
Run a stock exchange order book. Read commands from stdin, one per line (ignore blank lines):

- `BUY ID QTY PRICE` and `SELL ID QTY PRICE`: a limit order. ID is a word, QTY a whole number of at least 1, PRICE a positive decimal with at most 2 decimals (`12`, `12.5`, `12.50`).
- `MBUY ID QTY` and `MSELL ID QTY`: a market order.
- `CANCEL ID`: remove the resting order ID from the book; print `CANCELLED ID QTY` with the quantity that was still resting.
- `BOOK`: print the book.

Matching: an incoming buy trades with resting sells, cheapest price first and, at the same price, the oldest order first; a limit buy only trades with sells priced at or below its own price, a market buy with any sell. Selling is the mirror image (highest bid first, a limit sell only trades at or above its own price). Each fill prints `TRADE BUYID SELLID QTY PRICE`, at the resting order's price with 2 decimals. What is left of a limit order then rests in the book (keeping its arrival time); what is left of a market order is dropped and prints `DROPPED ID QTY`.

`BOOK` prints `asks` and then one line per sell price, lowest first: two spaces, the price (2 decimals), a space and the total resting quantity at that price; then `bids` and the buy prices the same way, highest first. A side with no orders prints two spaces and `-`.

Errors print one line and change nothing: `ERR line N: bad command` for a line that is not exactly one of the forms above (N is the line number in the input, counting blank lines), `ERR line N: duplicate ID` when a new order uses an ID that any earlier accepted order used, `ERR line N: unknown ID` when CANCEL names an order that is not resting.

After the last line print `volume Q value V`: the total traded quantity and the sum of quantity times price over all trades (2 decimals).
""", [
"""BUY b1 10 100.50
BUY b2 5 101
SELL s1 3 102
SELL s2 4 101.5
BOOK
SELL s3 12 100
BOOK
MBUY m1 10
BUY b3 2 99.99
CANCEL b3
CANCEL b3
BUY b1 1 1
""",
"""SELL a 5 10
SELL b 5 10
SELL c 5 9.5
BUY x 12 10
BOOK
MSELL y 3
BUY z 1 0
buy q 1 1
BUY w 1 1.234
BUY v 0 5
SELL u 2 abc
CANCEL

BOOK
""",
"""BOOK
MBUY a 5
SELL s 1 5.05
SELL t 1 5.05
SELL r 1 5.04
BUY big 10 5.05
SELL s2 4 5
BOOK
CANCEL big
BOOK
""",
])
def _(s):
    out = []; seen = set(); book = {"B": [], "S": []}; seq = 0; vol = 0; val = Decimal(0)
    qty_re = r"[1-9]\d*|0*[1-9]\d*"
    def price_ok(p): return re.fullmatch(r"\d+(\.\d{1,2})?", p) and Decimal(p) > 0
    for n, line in enumerate(s.split("\n"), 1):
        if not line.strip(): continue
        w = line.split()
        c = w[0]
        if c in ("BUY", "SELL") and len(w) == 4 and re.fullmatch(r"\d+", w[2]) and int(w[2]) > 0 and price_ok(w[3]):
            side, id_, q, p, market = c[0], w[1], int(w[2]), Decimal(w[3]), False
        elif c in ("MBUY", "MSELL") and len(w) == 3 and re.fullmatch(r"\d+", w[2]) and int(w[2]) > 0:
            side, id_, q, p, market = c[1], w[1], int(w[2]), None, True
        elif c == "CANCEL" and len(w) == 2:
            for sd in "BS":
                o = [x for x in book[sd] if x[2] == w[1]]
                if o:
                    book[sd].remove(o[0]); out.append(f"CANCELLED {w[1]} {o[0][3]}"); break
            else: out.append(f"ERR line {n}: unknown ID")
            continue
        elif c == "BOOK" and len(w) == 1:
            out.append("asks")
            for sd, rev in (("S", False), ("B", True)):
                if sd == "B": out.append("bids")
                agg = {}
                for x in book[sd]: agg[x[0]] = agg.get(x[0], 0) + x[3]
                if not agg: out.append("  -")
                for pr in sorted(agg, reverse=rev): out.append(f"  {d2(pr)} {agg[pr]}")
            continue
        else:
            out.append(f"ERR line {n}: bad command"); continue
        if id_ in seen: out.append(f"ERR line {n}: duplicate ID"); continue
        seen.add(id_)
        opp = book["S" if side == "B" else "B"]
        while q > 0 and opp:
            best = min(opp, key=lambda x: (x[0], x[1])) if side == "B" else min(opp, key=lambda x: (-x[0], x[1]))
            if not market and (best[0] > p if side == "B" else best[0] < p): break
            f = min(q, best[3])
            b, sl = (id_, best[2]) if side == "B" else (best[2], id_)
            out.append(f"TRADE {b} {sl} {f} {d2(best[0])}")
            vol += f; val += f * best[0]; q -= f; best[3] -= f
            if best[3] == 0: opp.remove(best)
        if q > 0:
            if market: out.append(f"DROPPED {id_} {q}")
            else: seq += 1; book[side].append([p, seq, id_, q])
    out.append(f"volume {vol} value {d2(val)}")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- mini_sql
@task("mini_sql", """
Run a tiny SQL database. Read statements from stdin, one per line (ignore blank lines). Keywords are upper case and words are separated by single spaces, exactly as below. Names are letters, digits and `_`, starting with a letter. A value is a whole number (optionally negative) or a string in single quotes (`''` inside it is one quote; it may contain spaces and commas).

- `CREATE TABLE T (COL TYPE, COL TYPE, ...)` where TYPE is `INT` or `TEXT`; prints `ok`. Creating a table that exists is `error: table T exists`.
- `INSERT INTO T VALUES (V, V, ...)`: one value per column, in order, of the column's type; prints `ok`.
- `SELECT COLS FROM T [WHERE COND] [ORDER BY COL [DESC]] [LIMIT N]`: COLS is `*` (all columns in table order) or column names separated by `, `. Prints the column names joined by ` | `, then each matching row the same way (strings without quotes), then `(N rows)`. Without ORDER BY, rows come in insertion order; ORDER BY sorts ascending (INT by number, TEXT by character code), DESC descending, and equal values keep insertion order either way. LIMIT N keeps the first N rows after sorting.
- `SELECT COUNT(*) FROM T [WHERE COND]`: prints the count.
- `UPDATE T SET COL = V [WHERE COND]`: prints `updated N`.
- `DELETE FROM T [WHERE COND]`: prints `deleted N`.

COND is one or more comparisons joined by ` AND `: `COL OP V` with OP one of `=`, `!=`, `<`, `>`, `<=`, `>=`.

Errors print one line and change nothing: `error: no table T`; `error: no column C` (the first unknown column in the statement, reading left to right); `error: type mismatch` when an INSERT has the wrong number of values or a value of the wrong type, or a comparison or SET value does not match its column's type; `error: syntax` for any line that is not one of the forms above (checked before anything else).
""", [
"""CREATE TABLE people (id INT, name TEXT, age INT, city TEXT)
INSERT INTO people VALUES (1, 'Ann', 34, 'Oslo')
INSERT INTO people VALUES (2, 'Bob', 27, 'Rome')
INSERT INTO people VALUES (3, 'O''Neil, Jo', 41, 'Oslo')
INSERT INTO people VALUES (4, 'Dee', 27, 'Lima')
SELECT * FROM people
SELECT name, age FROM people WHERE city = 'Oslo'
SELECT name FROM people WHERE age >= 27 AND age < 40 ORDER BY age DESC
SELECT id, name FROM people ORDER BY name LIMIT 2
SELECT COUNT(*) FROM people WHERE age = 27
UPDATE people SET city = 'Bergen' WHERE name = 'Ann'
SELECT name, city FROM people WHERE city != 'Oslo' ORDER BY city
DELETE FROM people WHERE age > 40
SELECT COUNT(*) FROM people
""",
"""CREATE TABLE t (a INT, b TEXT)
CREATE TABLE t (x INT)
INSERT INTO t VALUES (1)
INSERT INTO t VALUES ('x', 'y')
INSERT INTO t VALUES (-5, 'neg')
INSERT INTO u VALUES (1, 'a')
SELECT c FROM t
SELECT a FROM t WHERE b = 3
SELECT a FROM t WHERE zz = 3
select * from t
SELECT * FROM t WHERE a < 0
UPDATE t SET a = 'q'
UPDATE t SET b = 'it''s' WHERE a = -5
SELECT b FROM t
DELETE FROM t
SELECT * FROM t
SELECT a FROM t ORDER BY q
CREATE TABLE v (k INT, k2 BOOL)
""",
"""CREATE TABLE s (n INT, w TEXT)
INSERT INTO s VALUES (3, 'b')
INSERT INTO s VALUES (1, 'a')
INSERT INTO s VALUES (3, 'a')
INSERT INTO s VALUES (2, 'B')
INSERT INTO s VALUES (1, 'c, d')
SELECT * FROM s ORDER BY n
SELECT * FROM s ORDER BY n DESC LIMIT 3
SELECT w FROM s ORDER BY w
SELECT n FROM s WHERE w > 'a' AND n <= 2
SELECT COUNT(*) FROM s WHERE n = 9
SELECT * FROM s LIMIT 0
UPDATE s SET n = 0 WHERE n = 3
SELECT n, w FROM s WHERE n = 0
""",
])
def _(s):
    tables = {}; out = []
    NAME = r"[A-Za-z][A-Za-z0-9_]*"
    VAL = r"-?\d+|'(?:[^']|'')*'"
    COND = rf"{NAME} (?:=|!=|<=|>=|<|>) (?:{VAL})"
    WHERE = rf"(?: WHERE ({COND}(?: AND {COND})*))?"
    def val(v): return int(v) if v[0] != "'" else v[1:-1].replace("''", "'")
    def conds(c):
        if not c: return []
        return [re.fullmatch(rf"({NAME}) (=|!=|<=|>=|<|>) ({VAL})", x).groups() for x in re.findall(COND, c)]
    class E(Exception): pass
    def table(t):
        if t not in tables: raise E(f"error: no table {t}")
        return tables[t]
    def col(tb, c):
        names = [x[0] for x in tb["cols"]]
        if c not in names: raise E(f"error: no column {c}")
        return names.index(c)
    def check_conds(tb, cs):
        res = []
        for c, op, v in cs:
            i = col(tb, c); v = val(v)
            res.append((i, op, v))
        for i, op, v in res:
            if (tb["cols"][i][1] == "INT") != isinstance(v, int): raise E("error: type mismatch")
        return res
    def match(row, cs):
        for i, op, v in cs:
            a = row[i]
            ok = {"=": a == v, "!=": a != v, "<": a < v, ">": a > v, "<=": a <= v, ">=": a >= v}[op]
            if not ok: return False
        return True
    def show(v): return str(v)
    for line in s.split("\n"):
        if not line.strip(): continue
        try:
            m = re.fullmatch(rf"CREATE TABLE ({NAME}) \(({NAME} (?:INT|TEXT)(?:, {NAME} (?:INT|TEXT))*)\)", line)
            if m:
                if m[1] in tables: raise E(f"error: table {m[1]} exists")
                tables[m[1]] = {"cols": [tuple(x.split(" ")) for x in m[2].split(", ")], "rows": []}
                out.append("ok"); continue
            m = re.fullmatch(rf"INSERT INTO ({NAME}) VALUES \(((?:{VAL})(?:, (?:{VAL}))*)\)", line)
            if m:
                tb = table(m[1]); vs = [val(x) for x in re.findall(VAL, m[2])]
                if len(vs) != len(tb["cols"]) or any((c[1] == "INT") != isinstance(v, int) for c, v in zip(tb["cols"], vs)): raise E("error: type mismatch")
                tb["rows"].append(vs); out.append("ok"); continue
            m = re.fullmatch(rf"SELECT COUNT\(\*\) FROM ({NAME}){WHERE}", line)
            if m:
                tb = table(m[1]); cs = check_conds(tb, conds(m[2]))
                out.append(str(sum(1 for r in tb["rows"] if match(r, cs)))); continue
            m = re.fullmatch(rf"SELECT (\*|{NAME}(?:, {NAME})*) FROM ({NAME}){WHERE}(?: ORDER BY ({NAME})( DESC)?)?(?: LIMIT (\d+))?", line)
            if m:
                tb = table(m[2])
                names = [x[0] for x in tb["cols"]] if m[1] == "*" else m[1].split(", ")
                idx = [col(tb, c) for c in names]
                cs = check_conds(tb, conds(m[3]))
                oi = col(tb, m[4]) if m[4] else None
                rows = [r for r in tb["rows"] if match(r, cs)]
                if oi is not None:
                    # sorted() is stable, also with reverse=True, so equal values keep insertion order
                    rows = sorted(rows, key=lambda r: r[oi], reverse=bool(m[5]))
                if m[6] is not None: rows = rows[:int(m[6])]
                out.append(" | ".join(names))
                for r in rows: out.append(" | ".join(show(r[i]) for i in idx))
                out.append(f"({len(rows)} rows)"); continue
            m = re.fullmatch(rf"UPDATE ({NAME}) SET ({NAME}) = ({VAL}){WHERE}", line)
            if m:
                tb = table(m[1]); i = col(tb, m[2]); v = val(m[3])
                cs = check_conds(tb, conds(m[4]))
                if (tb["cols"][i][1] == "INT") != isinstance(v, int): raise E("error: type mismatch")
                n = 0
                for r in tb["rows"]:
                    if match(r, cs): r[i] = v; n += 1
                out.append(f"updated {n}"); continue
            m = re.fullmatch(rf"DELETE FROM ({NAME}){WHERE}", line)
            if m:
                tb = table(m[1]); cs = check_conds(tb, conds(m[2]))
                keep = [r for r in tb["rows"] if not match(r, cs)]
                out.append(f"deleted {len(tb['rows']) - len(keep)}"); tb["rows"] = keep; continue
            raise E("error: syntax")
        except E as e:
            out.append(str(e))
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- calc_lang
@task("calc_lang", """
Run programs in a small calculator language read from stdin, one statement per line (ignore blank lines and lines whose first non-space character is `#`). Numbers are 64-bit integers.

Statements:
- `let NAME = EXPR` sets a global variable.
- `fn NAME(P, P, ...) = EXPR` defines (or redefines) a function with zero or more parameters.
- `print EXPR, EXPR, ...` prints the values separated by single spaces.

Expressions, loosest first: comparisons `==`, `!=`, `<`, `>`, `<=`, `>=` (no chaining: `a < b < c` is a syntax error; they give 1 or 0); `+` and `-`; `*`, `/` and `%`; unary `-`; then numbers, names, `(EXPR)` and calls `NAME(EXPR, ...)`. Binary operators of the same level group left to right. `/` rounds toward zero and `%` has the sign of the left value (as in C). `if(C, A, B)` is built in: it evaluates C and then only A (when C is not 0) or only B. Names are letters, digits and `_`, starting with a letter; `let`, `fn`, `print` and `if` are not names. Spaces between tokens are optional.

Inside a function body a name is a parameter if it is one, otherwise a global variable as it is when the call runs. Functions may call themselves and each other, including functions defined later in the program, as long as they exist when the call runs.

Errors stop the current line only (nothing it would set changes, nothing it would print is printed) and print `error line N: MESSAGE`, N being the line number in the input: `syntax`, `unknown name X`, `unknown function F`, `F takes K arguments` (a call with the wrong count), `division by zero` (for `/` and `%`), `too deep` when a call would make more than 200 calls active at once. When a line has several errors, report the first one met while evaluating left to right; syntax errors are found before anything runs.
""", [
"""let x = 6
let y = x * 7 - 2
print x, y, -x + 3 * (2 - y)
print 7 / 2, -7 / 2, 7 % -3, -7 % 3
fn sq(n) = n * n
fn fact(n) = if(n < 2, 1, n * fact(n - 1))
print sq(5), fact(10), fact(20)
fn fib(n) = if(n < 2, n, fib(n - 1) + fib(n - 2))
print fib(15)
# comment line

print 3 < 4, 4 <= 3, 2 == 2, 2 != 2, 1 + 2 * 3 == 7
""",
"""print z
let a = 1
print a / 0
print a, 1 / (a - 1)
fn f(p) = p + q
print f(1)
let q = 10
print f(1), f(1, 2)
print g(2)
print 1 < 2 < 3
let = 4
print (1 + 2
print 1 +
fn deep(n) = if(n == 0, 0, 1 + deep(n - 1))
print deep(150)
print deep(250)
print if(1, 5, 1 / 0), if(0, 1 / 0, 6)
let if = 3
fn z() = 42
print z(), z( )
print   2*-3 ,  --4
""",
"""fn even(n) = if(n == 0, 1, odd(n - 1))
fn odd(n) = if(n == 0, 0, even(n - 1))
print even(10), odd(7)
fn gcd(a, b) = if(b == 0, a, gcd(b, a % b))
print gcd(1071, 462), gcd(17, 5)
let r = gcd(84, 36)
print r * r % 7
fn add3(a, b, c) = a + b + c
print add3(1, 2, 3), add3(1, 2)
fn power(b, e) = if(e == 0, 1, b * power(b, e - 1))
print power(2, 40), power(3, 3) - 30
let x = 5
fn usex(x) = x * 2
print usex(1), x
""",
])
def _(s):
    out = []; G = {}; F = {}
    class Err(Exception): pass
    def tokenize(t):
        toks = []; i = 0
        while i < len(t):
            c = t[i]
            if c == " ": i += 1; continue
            m = re.match(r"\d+|[A-Za-z][A-Za-z0-9_]*|==|!=|<=|>=|[-+*/%()<>,=]", t[i:])
            if not m: raise Err("syntax")
            toks.append(m.group()); i += len(m.group())
        return toks
    KW = {"let", "fn", "print", "if"}
    def isname(x): return re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", x) and x not in KW
    # parser -> tree
    def parse_expr(toks, pos):
        a, pos = parse_add(toks, pos)
        if pos < len(toks) and toks[pos] in ("==", "!=", "<", ">", "<=", ">="):
            op = toks[pos]; b, pos = parse_add(toks, pos + 1)
            if pos < len(toks) and toks[pos] in ("==", "!=", "<", ">", "<=", ">="): raise Err("syntax")
            a = ("bin", op, a, b)
        return a, pos
    def parse_add(toks, pos):
        a, pos = parse_mul(toks, pos)
        while pos < len(toks) and toks[pos] in "+-" and toks[pos] in ("+", "-"):
            op = toks[pos]; b, pos = parse_mul(toks, pos + 1); a = ("bin", op, a, b)
        return a, pos
    def parse_mul(toks, pos):
        a, pos = parse_un(toks, pos)
        while pos < len(toks) and toks[pos] in ("*", "/", "%"):
            op = toks[pos]; b, pos = parse_un(toks, pos + 1); a = ("bin", op, a, b)
        return a, pos
    def parse_un(toks, pos):
        if pos < len(toks) and toks[pos] == "-":
            a, pos = parse_un(toks, pos + 1); return ("neg", a), pos
        return parse_atom(toks, pos)
    def parse_atom(toks, pos):
        if pos >= len(toks): raise Err("syntax")
        t = toks[pos]
        if t.isdigit(): return ("num", int(t)), pos + 1
        if t == "(":
            a, pos = parse_expr(toks, pos + 1)
            if pos >= len(toks) or toks[pos] != ")": raise Err("syntax")
            return a, pos + 1
        if isname(t) or t == "if":
            if pos + 1 < len(toks) and toks[pos + 1] == "(":
                args = []; pos += 2
                if pos < len(toks) and toks[pos] == ")": return ("call", t, args), pos + 1
                while True:
                    a, pos = parse_expr(toks, pos); args.append(a)
                    if pos < len(toks) and toks[pos] == ",": pos += 1; continue
                    if pos < len(toks) and toks[pos] == ")": return ("call", t, args), pos + 1
                    raise Err("syntax")
            if t == "if": raise Err("syntax")
            return ("var", t), pos + 1
        raise Err("syntax")
    def full(toks):
        a, pos = parse_expr(toks, 0)
        if pos != len(toks): raise Err("syntax")
        return a
    def ev(e, env, depth):
        k = e[0]
        if k == "num": return e[1]
        if k == "var":
            if env is not None and e[1] in env: return env[e[1]]
            if e[1] in G: return G[e[1]]
            raise Err(f"unknown name {e[1]}")
        if k == "neg": return -ev(e[1], env, depth)
        if k == "bin":
            a = ev(e[2], env, depth); b = ev(e[3], env, depth); op = e[1]
            if op == "+": return a + b
            if op == "-": return a - b
            if op == "*": return a * b
            if op in "/%":
                if b == 0: raise Err("division by zero")
                q = abs(a) // abs(b) * (1 if (a >= 0) == (b >= 0) else -1)
                return q if op == "/" else a - q * b
            return int({"==": a == b, "!=": a != b, "<": a < b, ">": a > b, "<=": a <= b, ">=": a >= b}[op])
        if k == "call":
            name, args = e[1], e[2]
            if name == "if":
                if len(args) != 3: raise Err("if takes 3 arguments")
                return ev(args[1], env, depth) if ev(args[0], env, depth) != 0 else ev(args[2], env, depth)
            if name not in F: raise Err(f"unknown function {name}")
            ps, body = F[name]
            if len(ps) != len(args): raise Err(f"{name} takes {len(ps)} arguments")
            vals = [ev(a, env, depth) for a in args]
            if depth + 1 > 200: raise Err("too deep")
            return ev(body, dict(zip(ps, vals)), depth + 1)
    for n, line in enumerate(s.split("\n"), 1):
        st = line.strip()
        if not st or st.startswith("#"): continue
        try:
            toks = tokenize(st)
            if toks[0] == "let":
                if len(toks) < 4 or not isname(toks[1]) or toks[2] != "=": raise Err("syntax")
                e = full(toks[3:]); G[toks[1]] = ev(e, None, 0)
            elif toks[0] == "fn":
                if len(toks) < 5 or not isname(toks[1]) or toks[2] != "(": raise Err("syntax")
                ps = []; pos = 3
                if toks[pos] != ")":
                    while True:
                        if pos >= len(toks) or not isname(toks[pos]): raise Err("syntax")
                        ps.append(toks[pos]); pos += 1
                        if pos < len(toks) and toks[pos] == ",": pos += 1; continue
                        break
                if pos >= len(toks) or toks[pos] != ")" or pos + 1 >= len(toks) or toks[pos + 1] != "=": raise Err("syntax")
                F[toks[1]] = (ps, full(toks[pos + 2:]))
            elif toks[0] == "print":
                es = []; pos = 1
                while True:
                    a, pos = parse_expr(toks, pos); es.append(a)
                    if pos < len(toks) and toks[pos] == ",": pos += 1; continue
                    if pos != len(toks): raise Err("syntax")
                    break
                vals = [ev(e, None, 0) for e in es]
                out.append(" ".join(map(str, vals)))
            else: raise Err("syntax")
        except Err as e:
            out.append(f"error line {n}: {e}")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- fifo_stock
@task("fifo_stock", """
Keep stock for a warehouse using first-in first-out costing. Read lines from stdin (ignore blank lines), each `DATE OP SKU QTY [PRICE]` with DATE as `YYYY-MM-DD`, QTY a whole number of at least 1 and PRICE a decimal with at most 2 decimals:

- `RECEIVE SKU QTY COST`: a new lot of QTY units, each costing COST.
- `SHIP SKU QTY PRICE`: sell QTY units at PRICE each. Units leave from the oldest lots first; the cost of goods sold is the sum of the costs of the units that left, and the profit is the revenue (QTY times PRICE) minus that cost.
- `LOSS SKU QTY`: QTY units are lost, taken from the oldest lots first; their cost counts as a loss.

All amounts are exact. Errors print one line, change nothing, and processing goes on: `line N: bad line` for a line not in one of these forms, `line N: date out of order` for a date earlier than the latest date accepted so far, `line N: short SKU need Q have H` when a SHIP or LOSS asks for more units than are in stock. N is the line number in the input, counting blank lines.

After the last line print, for each SKU that ever had a lot, in name order: `SKU on_hand Q value V cogs C revenue R profit P loss L` (V is the cost of the units still in stock; amounts with 2 decimals). Then `months` and, for every month with a SHIP or LOSS, in order, `  YYYY-MM profit P loss L`, where profit and loss add up over all SKUs. Then `total profit P loss L net N` with N = profit minus loss.
""", [
"""2026-01-02 RECEIVE bolt 100 0.10
2026-01-03 RECEIVE bolt 50 0.12
2026-01-05 RECEIVE nut 10 1.25
2026-01-10 SHIP bolt 120 0.25
2026-01-11 SHIP nut 11 2.00
2026-01-20 LOSS bolt 10
2026-02-01 RECEIVE nut 5 1.40
2026-02-03 SHIP nut 12 2.10
2026-02-28 SHIP bolt 20 0.30
""",
"""2026-03-01 RECEIVE gear 3 9.99
2026-02-01 RECEIVE gear 1 1
2026-03-01 RECEIVE gear 0 5
2026-03-02 SHIP gear 2 12.5
2026-03-02 SELL gear 1 3
2026-3-04 SHIP gear 1 1

2026-03-05 RECEIVE gear 2 10.005
2026-03-06 LOSS gear 1 4
2026-03-07 LOSS gear 1
2026-04-01 SHIP gear 1 0.01
2026-04-02 SHIP gear 1 20
""",
"""2026-05-01 RECEIVE a 1 1.00
2026-05-01 RECEIVE b 2 2.50
2026-05-02 RECEIVE a 2 1.50
2026-05-03 SHIP a 3 1.20
2026-05-04 RECEIVE a 4 0.75
2026-06-09 SHIP a 1 0.75
2026-06-10 LOSS b 2
2026-06-11 LOSS b 1
""",
])
def _(s):
    lots = {}; stats = {}; months = {}; last = None; out = []
    D = Decimal
    for n, line in enumerate(s.split("\n"), 1):
        if not line.strip(): continue
        w = line.split(" ")
        okd = len(w) >= 4 and re.fullmatch(r"\d{4}-\d{2}-\d{2}", w[0])
        if okd:
            try: datetime.date.fromisoformat(w[0])
            except ValueError: okd = False
        price = lambda x: re.fullmatch(r"\d+(\.\d{1,2})?", x)
        qok = len(w) >= 4 and re.fullmatch(r"\d+", w[3]) and int(w[3]) > 0
        form = okd and qok and ((w[1] in ("RECEIVE", "SHIP") and len(w) == 5 and price(w[4])) or (w[1] == "LOSS" and len(w) == 4))
        if not form: out.append(f"line {n}: bad line"); continue
        if last and w[0] < last: out.append(f"line {n}: date out of order"); continue
        op, sku, q = w[1], w[2], int(w[3])
        L = lots.setdefault(sku, []) if op == "RECEIVE" else lots.get(sku, [])
        if op == "RECEIVE":
            L.append([q, D(w[4])]); stats.setdefault(sku, [D(0), D(0), D(0)]); last = w[0]; continue
        have = sum(x[0] for x in L)
        if q > have: out.append(f"line {n}: short {sku} need {q} have {have}"); continue
        last = w[0]; cost = D(0); r = q
        while r:
            t = min(r, L[0][0]); cost += t * L[0][1]; L[0][0] -= t; r -= t
            if L[0][0] == 0: L.pop(0)
        st = stats[sku]; m = months.setdefault(w[0][:7], [D(0), D(0)])
        if op == "SHIP":
            rev = q * D(w[4]); st[0] += cost; st[1] += rev; m[0] += rev - cost
        else:
            st[2] += cost; m[1] += cost
    tp = D(0); tl = D(0)
    for sku in sorted(stats):
        c, r, l = stats[sku]; L = lots[sku]
        out.append(f"{sku} on_hand {sum(x[0] for x in L)} value {d2(sum((x[0] * x[1] for x in L), D(0)))} cogs {d2(c)} revenue {d2(r)} profit {d2(r - c)} loss {d2(l)}")
        tp += r - c; tl += l
    out.append("months")
    for m in sorted(months): out.append(f"  {m} profit {d2(months[m][0])} loss {d2(months[m][1])}")
    out.append(f"total profit {d2(tp)} loss {d2(tl)} net {d2(tp - tl)}")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- library (multi-file)
@task("library", """
Run a lending library. Read commands from stdin, one per line (ignore blank lines). Every command starts with a date `YYYY-MM-DD`; dates never go backwards.

- `DATE BOOK ID COPIES TITLE...`: add a book with COPIES copies (TITLE is the rest of the line). Prints `added ID`.
- `DATE MEMBER ID TIER NAME...`: add a member; TIER is `basic` (at most 2 books at once, 14-day loans, late fee 0.25 a day) or `plus` (5 books, 28 days, 0.10 a day). Prints `welcome NAME`.
- `DATE LOAN MEMBER BOOK`: lend a copy. Prints `loan MEMBER BOOK due DUE` (DUE = DATE plus the loan days).
- `DATE RETURN MEMBER BOOK`: return the member's copy. If DATE is after the due date the fee is days late times the daily fee, at most 5.00 per loan, added to what the member owes. Prints `returned MEMBER BOOK` and, if there was a fee, ` fee F` on the same line (2 decimals).
- `DATE RESERVE MEMBER BOOK`: join the book's waiting list. Prints `reserved MEMBER BOOK position P` (1 is first).
- `DATE PAY MEMBER AMOUNT`: pay toward what the member owes (AMOUNT has at most 2 decimals; any extra is kept as credit, so what the member owes can go below 0). Prints `paid MEMBER owes O`.
- `DATE STATUS MEMBER`: prints `MEMBER NAME owes O loans: B1 B2 ...` with the member's books in the order they were lent (`loans: -` when none).

Holds: when a copy comes back and the book has a waiting list, the copy is held for the first member on the list instead of going back on the shelf. That member is taken off the list and prints nothing; the hold lasts 3 days (a return on day D can be collected through day D+3). Before each command, every hold whose last day is before the command's date expires and the copy goes to the next member on the list (a new 3-day hold from the day after the old hold's last day), or back on the shelf. Print `hold expired MEMBER BOOK` for each expired hold, in order of the hold's last day and then book ID.

A LOAN is refused, printing `refused MEMBER BOOK: REASON`, with the first reason that applies in this order: `unknown member`, `unknown book`, `owes O` (the member owes more than 2.00), `limit` (already has the tier's number of books), `already has it`, `none available` (no copy on the shelf and no hold for this member). A hold for the member is used before a shelf copy. RESERVE is refused (same output form) with `unknown member`, `unknown book`, `already has it`, `already reserved`, or `copy available` when a copy is on the shelf. RETURN of a book the member does not have prints `refused MEMBER BOOK: not on loan`; PAY and STATUS for an unknown member print `refused MEMBER: unknown member`. A second BOOK or MEMBER with an existing ID prints `duplicate ID`.
""", [
"""2026-01-01 BOOK b1 1 Dune
2026-01-01 BOOK b2 2 The Hobbit
2026-01-01 MEMBER ann basic Ann Lee
2026-01-01 MEMBER bob plus Bob Ray
2026-01-02 LOAN ann b1
2026-01-02 LOAN bob b1
2026-01-02 RESERVE bob b1
2026-01-03 RESERVE ann b1
2026-01-03 LOAN ann b2
2026-01-03 LOAN ann b9
2026-01-04 LOAN ann b2
2026-01-20 RETURN ann b1
2026-01-21 LOAN ann b2
2026-01-23 LOAN bob b1
2026-01-25 STATUS ann
2026-01-25 STATUS bob
2026-02-20 RETURN ann b2
2026-02-20 STATUS ann
2026-02-20 LOAN ann b1
2026-02-21 PAY ann 6
2026-02-21 STATUS ann
2026-02-22 MEMBER ann plus Ann Two
2026-02-22 RETURN bob b1
2026-02-22 RETURN bob b1
""",
"""2026-03-01 BOOK x 1 Rare Book
2026-03-01 MEMBER a basic A
2026-03-01 MEMBER b basic B
2026-03-01 MEMBER c plus C
2026-03-01 RESERVE a x
2026-03-01 LOAN a x
2026-03-02 RESERVE b x
2026-03-02 RESERVE c x
2026-03-02 RESERVE c x
2026-03-03 RETURN a x
2026-03-04 LOAN c x
2026-03-10 STATUS b
2026-03-10 LOAN c x
2026-03-10 STATUS c
2026-03-15 RETURN c x
2026-03-30 LOAN a x
2026-03-30 PAY zed 1
2026-03-30 STATUS a
""",
"""2026-04-01 BOOK p 3 Poems
2026-04-01 MEMBER m basic Mo
2026-04-01 LOAN m p
2026-04-01 LOAN m p
2026-04-02 BOOK q 1 Quiet
2026-04-02 LOAN m q
2026-05-30 RETURN m q
2026-05-30 RETURN m p
2026-05-30 LOAN m q
2026-05-30 PAY m 3.10
2026-05-30 LOAN m q
2026-05-31 PAY m 1.95
2026-05-31 STATUS m
""",
], files=["main", "catalog", "rules"])
def _(s):
    D = Decimal; out = []
    books = {}; members = {}; holds = []  # holds: [last_day, book, member]
    TIER = {"basic": (2, 14, D("0.25")), "plus": (5, 28, D("0.10"))}
    def day(x): return datetime.date.fromisoformat(x)
    def expire(today):
        while True:
            exp = sorted([h for h in holds if h[0] < today], key=lambda h: (h[0], h[1]))
            if not exp: return
            h = exp[0]; holds.remove(h)
            out.append(f"hold expired {h[2]} {h[1]}")
            b = books[h[1]]
            if b["wait"]:
                # the next member's hold runs for 3 days from the day after the old hold's last day
                holds.append([h[0] + datetime.timedelta(days=3), h[1], b["wait"].pop(0)])
            else: b["shelf"] += 1
    for line in s.split("\n"):
        if not line.strip(): continue
        w = line.split(" ")
        today = day(w[0]); op = w[1]
        expire(today)
        if op == "BOOK":
            if w[2] in books: out.append("duplicate ID"); continue
            books[w[2]] = {"shelf": int(w[3]), "wait": [], "title": " ".join(w[4:])}; out.append(f"added {w[2]}")
        elif op == "MEMBER":
            if w[2] in members: out.append("duplicate ID"); continue
            members[w[2]] = {"tier": w[3], "name": " ".join(w[4:]), "owes": D(0), "loans": []}; out.append(f"welcome {' '.join(w[4:])}")
        elif op == "LOAN":
            m, b = w[2], w[3]
            def why():
                if m not in members: return "unknown member"
                if b not in books: return "unknown book"
                M = members[m]
                if M["owes"] > 2: return f"owes {d2(M['owes'])}"
                if len(M["loans"]) >= TIER[M["tier"]][0]: return "limit"
                if any(x[0] == b for x in M["loans"]): return "already has it"
                if not any(h[1] == b and h[2] == m for h in holds) and books[b]["shelf"] == 0: return "none available"
            r = why()
            if r: out.append(f"refused {m} {b}: {r}"); continue
            hh = [h for h in holds if h[1] == b and h[2] == m]
            if hh: holds.remove(hh[0])
            else: books[b]["shelf"] -= 1
            due = today + datetime.timedelta(days=TIER[members[m]["tier"]][1])
            members[m]["loans"].append((b, due)); out.append(f"loan {m} {b} due {due}")
        elif op == "RETURN":
            m, b = w[2], w[3]
            M = members.get(m)
            L = [x for x in M["loans"] if x[0] == b] if M else []
            if not L: out.append(f"refused {m} {b}: not on loan"); continue
            M["loans"].remove(L[0]); late = (today - L[0][1]).days
            msg = f"returned {m} {b}"
            if late > 0:
                fee = min(D(5), late * TIER[M["tier"]][2]); M["owes"] += fee; msg += f" fee {d2(fee)}"
            out.append(msg)
            B = books[b]
            if B["wait"]: holds.append([today + datetime.timedelta(days=3), b, B["wait"].pop(0)])
            else: B["shelf"] += 1
        elif op == "RESERVE":
            m, b = w[2], w[3]
            def why():
                if m not in members: return "unknown member"
                if b not in books: return "unknown book"
                if any(x[0] == b for x in members[m]["loans"]): return "already has it"
                if m in books[b]["wait"] or any(h[1] == b and h[2] == m for h in holds): return "already reserved"
                if books[b]["shelf"] > 0: return "copy available"
            r = why()
            if r: out.append(f"refused {m} {b}: {r}"); continue
            books[b]["wait"].append(m); out.append(f"reserved {m} {b} position {len(books[b]['wait'])}")
        elif op == "PAY":
            m = w[2]
            if m not in members: out.append(f"refused {m}: unknown member"); continue
            members[m]["owes"] -= D(w[3]); out.append(f"paid {m} owes {d2(members[m]['owes'])}")
        elif op == "STATUS":
            m = w[2]
            if m not in members: out.append(f"refused {m}: unknown member"); continue
            M = members[m]; ls = " ".join(x[0] for x in M["loans"]) or "-"
            out.append(f"{m} {M['name']} owes {d2(M['owes'])} loans: {ls}")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- project_plan (multi-file)
@task("project_plan", """
Plan a project. Read stdin: first a line `workers K` (K at least 1), then one task per line (ignore blank lines): `NAME DAYS` or `NAME DAYS after A, B, ...`. NAME is letters, digits and `_`; DAYS is a whole number of at least 1; the names after `after` are tasks that must finish before this one starts.

First check the input and stop after printing the first problem found, if any: `line N: bad line` (a line not in these forms, including the first line; N counts blank lines), `line N: duplicate NAME` (a task named twice; the later line is reported), then, after reading everything, `unknown NAME` for the first missing dependency (in input order of the tasks that name it, then the order they name it), then `cycle` if the dependencies form a loop.

Otherwise print two plans.

1. `unlimited` and then, with no limit on workers, each task in input order as `  NAME START-END` where START is the earliest day it can start (the latest END of its dependencies, or 0) and END = START + DAYS. Then `length L` (the largest END) and `critical A > B > C`: a longest chain of tasks, from a task starting at 0 to one ending at L, each depending on the one before it with no gap (END of one equals START of the next). Where several tasks could come next in the chain, or start it, pick the one that comes first in the input.

2. `workers K` and then the same lines for a schedule with K workers, made like this: at each day T from 0, first finish the tasks that end at T; then, while a worker is free, start the ready task (all its dependencies finished) with the largest `tail` (its DAYS plus the largest tail of the tasks that depend on it, 0 if none), ties to the first in the input. Print tasks in input order as `  NAME START-END`, then `length L`.
""", [
"""workers 2
design 3
db 4 after design
api 5 after design
ui 6 after design
tests 2 after db, api
docs 1
launch 1 after tests, ui, docs
""",
"""workers 1
a 2
b 3
c 1 after a
d 2 after b, c
""",
"""workers 3
x 1 after y

y 2 after z
z 3 after x
""",
"""workers 2
a 1
b 2 after a
c 0 after a
""",
"""workers 2
a 1
b 2 after q
c 1 after a, r
""",
"""workers 4
a 5
b 5
a 1
""",
"""workers 3
s1 2
s2 2
s3 2
s4 2
m1 3 after s1, s2
m2 1 after s3
m3 4 after s4
end 2 after m1, m2, m3
""",
], files=["main", "graph", "schedule"])
def _(s):
    lines = s.split("\n")
    if lines and lines[-1] == "": lines.pop()
    out = []
    m = re.fullmatch(r"workers ([1-9]\d*)", lines[0]) if lines else None
    if not m: return "line 1: bad line\n"
    K = int(m[1]); tasks = {}; order = []
    for n, line in enumerate(lines[1:], 2):
        if not line.strip(): continue
        mm = re.fullmatch(r"(\w+) ([1-9]\d*)(?: after (\w+(?:, \w+)*))?", line)
        if not mm or mm[1] == "after": return f"line {n}: bad line\n"
        if mm[1] in tasks: return f"line {n}: duplicate {mm[1]}\n"
        tasks[mm[1]] = (int(mm[2]), mm[3].split(", ") if mm[3] else []); order.append(mm[1])
    for t in order:
        for d in tasks[t][1]:
            if d not in tasks: return f"unknown {d}\n"
    state = {}
    def dfs(t):
        state[t] = 1
        for d in tasks[t][1]:
            if state.get(d) == 1: return True
            if d not in state and dfs(d): return True
        state[t] = 2; return False
    for t in order:
        if t not in state and dfs(t): return "cycle\n"
    es = {}
    def start(t):
        if t not in es: es[t] = max((start(d) + tasks[d][0] for d in tasks[t][1]), default=0)
        return es[t]
    out.append("unlimited")
    for t in order: out.append(f"  {t} {start(t)}-{start(t) + tasks[t][0]}")
    L = max(start(t) + tasks[t][0] for t in order); out.append(f"length {L}")
    # critical chain: a task is on a longest path if start + tail == L
    succ = {t: [u for u in order if t in tasks[u][1]] for t in order}
    tl = {}
    def tail(t):
        if t not in tl: tl[t] = tasks[t][0] + max((tail(u) for u in succ[t]), default=0)
        return tl[t]
    cur = next(t for t in order if start(t) == 0 and tail(t) == L)
    chain = [cur]
    while start(cur) + tasks[cur][0] != L:
        e = start(cur) + tasks[cur][0]
        cur = next(u for u in order if cur in tasks[u][1] and start(u) == e and start(u) + tail(u) == L)
        chain.append(cur)
    out.append("critical " + " > ".join(chain))
    out.append(f"workers {K}")
    done = set(); running = {}; st = {}; T = 0
    while len(done) < len(order):
        for t in [x for x in running if running[x] == T]: done.add(t); del running[t]
        ready = [t for t in order if t not in st and all(d in done for d in tasks[t][1])]
        ready.sort(key=lambda t: (-tail(t), order.index(t)))
        while len(running) < K and ready:
            t = ready.pop(0); st[t] = T; running[t] = T + tasks[t][0]
        if len(done) == len(order): break
        T = min(running.values())
    for t in order: out.append(f"  {t} {st[t]}-{st[t] + tasks[t][0]}")
    out.append(f"length {max(st[t] + tasks[t][0] for t in order)}")
    return "\n".join(out) + "\n"

for name, (prompt, inputs, fn) in T.items():
    d = f"tasks/{name}"; os.makedirs(f"{d}/tests", exist_ok=True)
    if name in FILES:
        fs = FILES[name]
        prompt += "\n" + MULTI.format(n=len(fs), files=", ".join(f"`{f}`" for f in fs),
            roles={"library": "`catalog` defines the book and member records and the library state that holds them; `rules` has the lending rules (loans, returns, fees, holds); `main` reads the commands and prints. Keep the state in `main` (or in values it creates) and pass it to the functions that need it.",
                   "project_plan": "`graph` parses and checks the tasks and works out the unlimited plan and the critical chain; `schedule` makes the plan for K workers; `main` reads stdin and prints."}[name]).rstrip()
        open(f"{d}/files", "w").write(" ".join(fs) + "\n")
    open(f"{d}/prompt.md", "w").write(prompt + "\n")
    for i, inp in enumerate(inputs):
        open(f"{d}/tests/{i}.in", "w").write(inp)
        open(f"{d}/tests/{i}.out", "w").write(fn(inp))
print(len(T), "tasks")
