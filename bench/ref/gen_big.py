# Bigger, more realistic tasks (written 2026-10-31, never used to tune Axon).
# Writes tasks/<name>/{prompt.md,tests/N.in,N.out}
import os, json, re
from decimal import Decimal, ROUND_HALF_EVEN
T = {}
def task(name, prompt, inputs):
    def deco(fn):
        T[name] = (prompt.strip(), inputs, fn); return fn
    return deco

# ---------------------------------------------------------------- kv_store
@task("kv_store", """
Implement an in-memory key-value store with nested transactions. Each stdin line is one command; print output only where stated.

- `SET k v`: set key k to string v.
- `GET k`: print the value, or `NULL` if k is not set.
- `DEL k`: remove k. Print `NOT FOUND` if it was not set.
- `INCR k n`: add the integer n to k's value (a missing key counts as 0) and print the new value. If the current value is not an integer, print `ERR not an integer` and change nothing.
- `COUNT v`: print how many keys currently have the value v.
- `KEYS p`: print all keys starting with the prefix p, sorted, separated by single spaces, or `(empty)` if there are none.
- `BEGIN`: open a transaction. Transactions can be nested.
- `ROLLBACK`: undo every change made since the most recent open BEGIN and close that transaction. Print `NO TRANSACTION` if none is open.
- `COMMIT`: close all open transactions, keeping their changes. Print `NO TRANSACTION` if none is open.
- Anything else: print `ERR unknown command`.

Commands are uppercase and arguments have no spaces. Ignore blank lines.
""", [
"SET a 10\nSET b 10\nCOUNT 10\nGET a\nDEL c\nINCR a 5\nGET a\nCOUNT 10\nBEGIN\nSET a 1\nBEGIN\nDEL b\nGET b\nROLLBACK\nGET b\nGET a\nROLLBACK\nGET a\nROLLBACK\n",
"BEGIN\nSET x hello\nINCR x 1\nINCR y -3\nBEGIN\nSET user1 a\nSET user2 b\nSET usual c\nKEYS user\nKEYS us\nKEYS z\nCOMMIT\nROLLBACK\nGET y\nCOUNT a\nFLY away\nGET user2\n",
"SET k 1\nBEGIN\nDEL k\nSET k 2\nBEGIN\nINCR k 40\nCOMMIT\nGET k\nCOMMIT\n\nBEGIN\nSET n 1\nBEGIN\nSET n 2\nBEGIN\nSET n 3\nROLLBACK\nGET n\nROLLBACK\nGET n\nROLLBACK\nGET n\n",
])
def _(s):
    db = {}; stack = []; out = []
    def setk(k, v):
        if stack: stack[-1].append((k, db.get(k)))
        if v is None: db.pop(k, None)
        else: db[k] = v
    for l in s.split("\n"):
        p = l.split()
        if not p: continue
        c = p[0]
        if c == "SET": setk(p[1], p[2])
        elif c == "GET": out.append(db.get(p[1], "NULL"))
        elif c == "DEL":
            if p[1] in db: setk(p[1], None)
            else: out.append("NOT FOUND")
        elif c == "INCR":
            cur = db.get(p[1], "0")
            if not re.fullmatch(r"-?\d+", cur): out.append("ERR not an integer"); continue
            v = str(int(cur) + int(p[2])); setk(p[1], v); out.append(v)
        elif c == "COUNT": out.append(str(sum(1 for v in db.values() if v == p[1])))
        elif c == "KEYS":
            ks = sorted(k for k in db if k.startswith(p[1])); out.append(" ".join(ks) if ks else "(empty)")
        elif c == "BEGIN": stack.append([])
        elif c == "ROLLBACK":
            if not stack: out.append("NO TRANSACTION"); continue
            for k, old in reversed(stack.pop()):
                if old is None: db.pop(k, None)
                else: db[k] = old
        elif c == "COMMIT":
            if not stack: out.append("NO TRANSACTION"); continue
            stack.clear()
        else: out.append("ERR unknown command")
    return "".join(x + "\n" for x in out)

# ---------------------------------------------------------------- bank_api
@task("bank_api", """
Stdin holds HTTP-style requests for a small bank API, one per line: `METHOD PATH` optionally followed by a space and a JSON object body. For each request print one line: the status code, a space, and a JSON response body written compactly (no spaces, keys in the order shown below).

Accounts have an id (integers from 1, in creation order), an owner and a balance in cents (an integer).

- `POST /accounts {"owner": NAME}`: create an account with balance 0. Respond `201 {"id":ID,"owner":NAME,"balance":0}`. If owner is missing, not a string, or empty, respond `400 {"error":"owner required"}`.
- `GET /accounts/ID`: respond `200` with the account object as above, or `404 {"error":"no such account"}`.
- `POST /accounts/ID/deposit {"amount": N}`: N must be a positive integer, else `400 {"error":"bad amount"}`. Respond `200` with the updated account.
- `POST /accounts/ID/withdraw {"amount": N}`: as deposit, but if N is more than the balance respond `409 {"error":"insufficient funds"}`.
- `POST /transfer {"from": ID, "to": ID, "amount": N}`: both accounts must exist (else 404 as above), they must differ (else `400 {"error":"same account"}`), the amount is checked as above, then funds as above. Respond `200 {"from":BALANCE,"to":BALANCE}` with both new balances.
- `GET /accounts`: respond `200` with a JSON array of all accounts sorted by balance, highest first, then by id.
- Any other method and path: `404 {"error":"not found"}`. A body that is not valid JSON: `400 {"error":"bad json"}`.

Check in this order: route, then JSON body, then account existence, then the other rules. A route with a non-numeric ID is "not found". Names contain only letters and spaces.
""", [
'POST /accounts {"owner": "Ann"}\nPOST /accounts {"owner": "Bob"}\nPOST /accounts/1/deposit {"amount": 500}\nPOST /accounts/1/withdraw {"amount": 800}\nPOST /transfer {"from": 1, "to": 2, "amount": 200}\nGET /accounts\nGET /accounts/3\nGET /accounts/2\n',
'POST /accounts {}\nPOST /accounts {"owner": ""}\nPOST /accounts {"owner": 5}\nPOST /accounts {"owner": "Cy Young"}\nPOST /accounts/1/deposit {"amount": -5}\nPOST /accounts/1/deposit {"amount": 2.5}\nPOST /accounts/1/deposit {"amount": "7"}\nPOST /accounts/1/deposit {oops}\nDELETE /accounts/1\nGET /accounts/x\nPOST /transfer {"from": 1, "to": 1, "amount": 1}\nPOST /transfer {"from": 1, "to": 9, "amount": 1}\nGET /accounts\n',
'POST /accounts {"owner": "A"}\nPOST /accounts {"owner": "B"}\nPOST /accounts {"owner": "C"}\nPOST /accounts/2/deposit {"amount": 300}\nPOST /accounts/3/deposit {"amount": 300}\nPOST /accounts/1/deposit {"amount": 100}\nPOST /transfer {"from": 3, "to": 1, "amount": 301}\nPOST /transfer {"from": 3, "to": 1, "amount": 300}\nPOST /accounts/9/withdraw {"amount": 1}\nGET /accounts\n',
])
def _(s):
    accts = {}; out = []
    def show(a): return {"id": a["id"], "owner": a["owner"], "balance": a["balance"]}
    def r(code, body): out.append(f"{code} {json.dumps(body, separators=(',', ':'))}")
    def amount_ok(b):
        n = b.get("amount"); return isinstance(n, int) and not isinstance(n, bool) and n > 0
    for l in s.split("\n"):
        if not l.strip(): continue
        parts = l.split(" ", 2); m, path = parts[0], parts[1]; raw = parts[2] if len(parts) > 2 else None
        routes = [("POST", r"/accounts"), ("GET", r"/accounts/(\d+)"), ("POST", r"/accounts/(\d+)/deposit"),
                  ("POST", r"/accounts/(\d+)/withdraw"), ("POST", r"/transfer"), ("GET", r"/accounts")]
        hit = None
        for i, (rm, rp) in enumerate(routes):
            mm = re.fullmatch(rp, path)
            if rm == m and mm: hit = (i, mm.groups()); break
        if not hit: r(404, {"error": "not found"}); continue
        body = {}
        if raw is not None:
            try: body = json.loads(raw)
            except Exception: r(400, {"error": "bad json"}); continue
            if not isinstance(body, dict): r(400, {"error": "bad json"}); continue
        i, g = hit
        if i == 0:
            o = body.get("owner")
            if not isinstance(o, str) or not o: r(400, {"error": "owner required"}); continue
            a = {"id": len(accts) + 1, "owner": o, "balance": 0}; accts[a["id"]] = a; r(201, show(a))
        elif i == 1:
            a = accts.get(int(g[0]))
            r(200, show(a)) if a else r(404, {"error": "no such account"})
        elif i in (2, 3):
            a = accts.get(int(g[0]))
            if not a: r(404, {"error": "no such account"}); continue
            if not amount_ok(body): r(400, {"error": "bad amount"}); continue
            n = body["amount"]
            if i == 3 and n > a["balance"]: r(409, {"error": "insufficient funds"}); continue
            a["balance"] += n if i == 2 else -n; r(200, show(a))
        elif i == 4:
            f, t = accts.get(body.get("from")), accts.get(body.get("to"))
            if not f or not t: r(404, {"error": "no such account"}); continue
            if f is t: r(400, {"error": "same account"}); continue
            if not amount_ok(body): r(400, {"error": "bad amount"}); continue
            n = body["amount"]
            if n > f["balance"]: r(409, {"error": "insufficient funds"}); continue
            f["balance"] -= n; t["balance"] += n; r(200, {"from": f["balance"], "to": t["balance"]})
        else:
            r(200, [show(a) for a in sorted(accts.values(), key=lambda a: (-a["balance"], a["id"]))])
    return "".join(x + "\n" for x in out)

# ---------------------------------------------------------------- spreadsheet
@task("spreadsheet", """
Stdin defines spreadsheet cells, one per line as `CELL = CONTENT` (for example `B2 = =A1*2`). Cells are a column letter A to Z and a row number 1 to 99. CONTENT is either an integer, or `=` followed by a formula.

Formulas use integers, cell references, `+ - * /` with the usual precedence (`*` and `/` before `+` and `-`, left to right), parentheses, unary minus, and the functions `SUM(range)`, `MIN(range)`, `MAX(range)` where a range is `A1:B3` (every cell in that rectangle). Spaces may appear anywhere in a formula. `/` is integer division rounding toward zero. An empty cell counts as 0.

Print a grid covering columns A to the last column used and rows 1 to the last row used (counting both defined cells and cells mentioned in formulas). The first line is a header: a tab, then the column letters separated by tabs. Then one line per row: the row number, then each cell's value, all separated by tabs. Empty cells print as nothing. A cell in a reference cycle, or depending on one, prints `#CYCLE`. A cell that divides by zero, or depends on one that does, prints `#DIV0` (if a cell has both problems, `#CYCLE` wins).
""", [
"A1 = 5\nA2 = 7\nA3 = =A1+A2*2\nB1 = =SUM(A1:A3)\nB2 = =(A1 + A2) * -2\nB3 = =MAX(A1:B2) - MIN(A1:A3)\nC1 = =B1/4\n",
"A1 = =B1+1\nB1 = =A1+1\nC1 = =A1*0\nA2 = 10\nB2 = =A2/0\nC2 = =B2+1\nD2 = =-7/2\nC3 = =SUM(A2:A2)+E4\n",
"B2 = =  3 * ( 4 + 2 ) - 10 / 3\nA1 = =B2\nC1 = =SUM(A1:B2) * 2\n",
])
def _(s):
    cells = {}
    used_cols, used_rows = set(), set()
    def note(ref): used_cols.add(ref[0]); used_rows.add(int(ref[1:]))
    for l in s.strip().split("\n"):
        k, v = l.split("=", 1); k = k.strip(); v = v.strip(); cells[k] = v; note(k)
        if v.startswith("="):
            for ref in re.findall(r"[A-Z]\d+", v): note(ref)
    cache = {}; visiting = set()
    class Cyc(Exception): pass
    class Div(Exception): pass
    def val(ref):
        if ref in cache:
            r = cache[ref]
            if r == "#CYCLE": raise Cyc()
            if r == "#DIV0": raise Div()
            return r
        if ref in visiting: raise Cyc()
        c = cells.get(ref)
        if c is None: return 0
        if not c.startswith("="): cache[ref] = int(c); return cache[ref]
        visiting.add(ref)
        try:
            res = ev(c[1:])
        except Cyc:
            visiting.discard(ref); cache[ref] = "#CYCLE"; raise
        except Div:
            visiting.discard(ref); cache[ref] = "#DIV0"; raise
        visiting.discard(ref); cache[ref] = res; return res
    def ev(f):
        toks = re.findall(r"[A-Z]+\(|[A-Z]\d+:[A-Z]\d+|[A-Z]\d+|\d+|[-+*/()]", f)
        pos = [0]
        def peek(): return toks[pos[0]] if pos[0] < len(toks) else None
        def take(): pos[0] += 1; return toks[pos[0] - 1]
        def expr():
            v = term()
            while peek() in ("+", "-"):
                op = take(); w = term(); v = v + w if op == "+" else v - w
            return v
        def term():
            v = unary()
            while peek() in ("*", "/"):
                op = take(); w = unary()
                if op == "*": v = v * w
                else:
                    if w == 0: raise Div()
                    q = abs(v) // abs(w); v = q if (v >= 0) == (w >= 0) else -q
            return v
        def unary():
            if peek() == "-": take(); return -unary()
            return atom()
        def atom():
            t = take()
            if t == "(": v = expr(); take(); return v
            if t.endswith("("):
                rng = take(); take(); a, b = rng.split(":")
                vals = [val(f"{chr(c)}{r}") for c in range(ord(a[0]), ord(b[0]) + 1) for r in range(int(a[1:]), int(b[1:]) + 1)]
                return {"SUM(": sum, "MIN(": min, "MAX(": max}[t](vals)
            if t[0].isdigit(): return int(t)
            return val(t)
        # Evaluate every referenced cell so a cycle anywhere wins over a division error.
        return expr()
    # Cycle check first: a cell in or depending on a cycle is #CYCLE even if a division by zero comes first.
    deps = {k: re.findall(r"[A-Z]\d+(?::[A-Z]\d+)?", v[1:]) if v.startswith("=") else [] for k, v in cells.items()}
    def expand(d):
        if ":" not in d: return [d]
        a, b = d.split(":")
        return [f"{chr(c)}{r}" for c in range(ord(a[0]), ord(b[0]) + 1) for r in range(int(a[1:]), int(b[1:]) + 1)]
    state = {}; cyc = set()
    def dfs(k, path):
        if state.get(k) == 1:
            cyc.update(path[path.index(k):]); return True
        if state.get(k) == 2: return k in cyc
        state[k] = 1; path.append(k); bad = False
        for d in deps.get(k, []):
            for e in expand(d):
                if dfs(e, path): bad = True
        path.pop(); state[k] = 2
        if bad: cyc.add(k)
        return bad
    for k in cells: dfs(k, [])
    cols = [chr(c) for c in range(ord("A"), ord(max(used_cols)) + 1)]
    out = ["\t" + "\t".join(cols)]
    for r in range(1, max(used_rows) + 1):
        row = [str(r)]
        for c in cols:
            k = f"{c}{r}"
            if k not in cells: row.append(""); continue
            if k in cyc: row.append("#CYCLE"); continue
            try: row.append(str(val(k)))
            except Div: row.append("#DIV0")
            except Cyc: row.append("#CYCLE")
        out.append("\t".join(row))
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- markdown
@task("markdown", """
Convert a small subset of Markdown on stdin to HTML on stdout.

Blocks (separated by blank lines or by a change of block type):
- `# Title` to `###### Title`: a heading `<h1>`..`<h6>`. Its id is the text lowercased, with every run of characters other than a-z and 0-9 replaced by one `-`, and leading or trailing `-` removed. Output `<h2 id="the-id">Text</h2>`. If an id was already used, add `-2`, `-3`, and so on.
- Lines starting with `- `: items of one `<ul>`. Lines starting with `1. ` (any number then `. `): items of one `<ol>`. Each item is `<li>...</li>` on its own line, between `<ul>`/`</ul>` lines.
- A line of exactly three backticks starts a code block, which ends at the next such line. Output `<pre><code>`, the lines in between (HTML-escaped, nothing else changed) joined by newlines, then `</code></pre>`, all as written: `<pre><code>` directly followed by the first line, and `</code></pre>` directly after the last line.
- Any other consecutive non-blank lines form a paragraph: `<p>` + the lines joined with a single space + `</p>`.

Inline (in headings, items and paragraphs): first HTML-escape `&`, `<`, `>` (as `&amp;` `&lt;` `&gt;`), then `` `code` `` becomes `<code>code</code>` (its content is not further formatted), `**bold**` becomes `<strong>bold</strong>`, `*em*` becomes `<em>em</em>`, and `[text](url)` becomes `<a href="url">text</a>`.

Finally, if there are at least two headings, print a table of contents before everything else: `<nav>` on a line, then for each heading a line `<a href="#id">Text</a>` (Text with inline formatting removed, that is the raw heading text with the markers `` ` `` and `*` deleted, HTML-escaped), then `</nav>`. Each block is printed on its own line(s), with nothing between blocks.
""", [
"# Intro\n\nSome *simple* text\nover two lines with **bold**.\n\n## Use `x < y`\n\n- one\n- [two](http://a.b/c)\n\n1. first\n2. second\n",
"Plain & simple.\n```\nlet a = <b>;\n  *not em*\n```\n# Intro\n# Intro\n### Q&A: what's up?\n",
"- only *a* list\n- **b** and `c*d*`\n",
])
def _(s):
    def esc(t): return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    def inline(t):
        t = esc(t); parts = re.split(r"(`[^`]*`)", t); o = []
        for p in parts:
            if p.startswith("`") and p.endswith("`") and len(p) >= 2: o.append(f"<code>{p[1:-1]}</code>"); continue
            p = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", p)
            p = re.sub(r"\*(.+?)\*", r"<em>\1</em>", p)
            p = re.sub(r"\[([^\]]*)\]\(([^)]*)\)", r'<a href="\2">\1</a>', p)
            o.append(p)
        return "".join(o)
    lines = s.split("\n"); out = []; heads = []; used = {}
    i = 0; para = []; lst = None
    def flush():
        nonlocal para, lst
        if para: out.append("<p>" + inline(" ".join(para)) + "</p>"); para = []
        if lst: out.append(f"<{lst[0]}>"); out.extend(f"<li>{inline(x)}</li>" for x in lst[1]); out.append(f"</{lst[0]}>"); lst = None
    while i < len(lines):
        l = lines[i]
        if l == "```":
            flush(); j = i + 1; body = []
            while j < len(lines) and lines[j] != "```": body.append(esc(lines[j])); j += 1
            out.append("<pre><code>" + "\n".join(body) + "</code></pre>"); i = j + 1; continue
        if not l.strip(): flush(); i += 1; continue
        m = re.match(r"(#{1,6}) (.*)", l)
        if m:
            flush(); text = m.group(2); base = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
            n = used.get(base, 0) + 1; used[base] = n; hid = base if n == 1 else f"{base}-{n}"
            heads.append((hid, esc(text.replace("`", "").replace("*", ""))))
            out.append(f'<h{len(m.group(1))} id="{hid}">{inline(text)}</h{len(m.group(1))}>'); i += 1; continue
        kind = "ul" if l.startswith("- ") else "ol" if re.match(r"\d+\. ", l) else None
        if kind:
            if para: flush()
            if lst and lst[0] != kind: flush()
            if not lst: lst = (kind, [])
            lst[1].append(l[2:] if kind == "ul" else l.split(". ", 1)[1]); i += 1; continue
        if lst: flush()
        para.append(l); i += 1
    flush()
    if len(heads) >= 2:
        out = ["<nav>"] + [f'<a href="#{h}">{t}</a>' for h, t in heads] + ["</nav>"] + out
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- invoices
@task("invoices", """
Stdin is one JSON object: `{"products": {...}, "customers": {...}, "orders": [...]}`.
- `products` maps a SKU to `{"name": str, "price": "12.50", "category": str}` (price is a decimal string).
- `customers` maps a customer id to `{"name": str, "tier": "gold" | "silver" | "basic", "country": str}`.
- `orders` is a list of `{"id": str, "customer": id, "lines": [{"sku": str, "qty": int}]}`.

For each order, in input order, compute an invoice:
- Each line's amount is price times qty. Unknown SKUs make the whole order invalid.
- Discount: gold customers get 10% off the subtotal, silver 5%, basic nothing. Additionally, any category with 3 or more units in the order (qty summed over its lines) gets an extra 5% off that category's lines, applied before the tier discount. Round the discount amounts (each one) to cents, half to even.
- Tax: 20% of the discounted total if the country is "UK", 8.25% if "US", otherwise 0. Round tax to cents, half to even.
- An unknown customer also makes the order invalid.

Print a JSON array (compact, no spaces) of invoices, each `{"id":...,"customer":NAME,"subtotal":"...","discount":"...","tax":"...","total":"..."}` with money as strings with exactly 2 decimals, where discount is the sum of all discounts and total = subtotal - discount + tax. An invalid order prints `{"id":...,"error":"unknown sku SKU"}` or `{"id":...,"error":"unknown customer ID"}` (customer is checked first; for skus name the first unknown one).
Then print a second line: the total revenue (sum of valid totals) as a string with 2 decimals, in the form `revenue 123.45`.
""", [
json.dumps({"products": {"p1": {"name": "Pen", "price": "1.20", "category": "office"}, "p2": {"name": "Pad", "price": "3.35", "category": "office"}, "t1": {"name": "Tea", "price": "4.99", "category": "food"}},
            "customers": {"c1": {"name": "Ann", "tier": "gold", "country": "UK"}, "c2": {"name": "Bob", "tier": "basic", "country": "US"}, "c3": {"name": "Cy", "tier": "silver", "country": "FR"}},
            "orders": [{"id": "o1", "customer": "c1", "lines": [{"sku": "p1", "qty": 2}, {"sku": "p2", "qty": 1}, {"sku": "t1", "qty": 1}]},
                       {"id": "o2", "customer": "c2", "lines": [{"sku": "t1", "qty": 3}]},
                       {"id": "o3", "customer": "c9", "lines": [{"sku": "zz", "qty": 1}]},
                       {"id": "o4", "customer": "c3", "lines": [{"sku": "p1", "qty": 1}, {"sku": "zz", "qty": 1}, {"sku": "yy", "qty": 1}]},
                       {"id": "o5", "customer": "c3", "lines": [{"sku": "p2", "qty": 5}]}]}, indent=1) + "\n",
json.dumps({"products": {"a": {"name": "A", "price": "0.05", "category": "x"}}, "customers": {"k": {"name": "Kim", "tier": "silver", "country": "US"}},
            "orders": [{"id": "1", "customer": "k", "lines": [{"sku": "a", "qty": 1}]}, {"id": "2", "customer": "k", "lines": [{"sku": "a", "qty": 3}]}, {"id": "3", "customer": "k", "lines": []}]}) + "\n",
])
def _(s):
    d = json.loads(s); P, C = d["products"], d["customers"]; out = []; rev = Decimal(0)
    def r2(x): return x.quantize(Decimal("0.01"), rounding=ROUND_HALF_EVEN)
    for o in d["orders"]:
        c = C.get(o["customer"])
        if not c: out.append({"id": o["id"], "error": f"unknown customer {o['customer']}"}); continue
        bad = next((l["sku"] for l in o["lines"] if l["sku"] not in P), None)
        if bad: out.append({"id": o["id"], "error": f"unknown sku {bad}"}); continue
        sub = Decimal(0); cat_amt = {}; cat_qty = {}
        for l in o["lines"]:
            p = P[l["sku"]]; a = Decimal(p["price"]) * l["qty"]; sub += a
            cat_amt[p["category"]] = cat_amt.get(p["category"], Decimal(0)) + a
            cat_qty[p["category"]] = cat_qty.get(p["category"], 0) + l["qty"]
        disc = Decimal(0)
        for k in cat_amt:
            if cat_qty[k] >= 3: disc += r2(cat_amt[k] * Decimal("0.05"))
        rate = {"gold": Decimal("0.10"), "silver": Decimal("0.05")}.get(c["tier"], Decimal(0))
        disc += r2((sub - disc) * rate)
        net = sub - disc
        tax = r2(net * {"UK": Decimal("0.20"), "US": Decimal("0.0825")}.get(c["country"], Decimal(0)))
        tot = net + tax; rev += tot
        out.append({"id": o["id"], "customer": c["name"], "subtotal": f"{sub:.2f}", "discount": f"{disc:.2f}", "tax": f"{tax:.2f}", "total": f"{tot:.2f}"})
    return json.dumps(out, separators=(",", ":")) + "\n" + f"revenue {rev:.2f}\n"

# ---------------------------------------------------------------- access_report
@task("access_report", """
Stdin is a web server access log. Each line looks like
`203.0.113.9 - - [10/Oct/2026:13:55:36 +0000] "GET /index.html HTTP/1.1" 200 2326 "-" "Mozilla/5.0"`:
IP, two dashes, a bracketed timestamp, a quoted request line `METHOD PATH PROTOCOL`, the status code, the response size in bytes (or `-` meaning 0), then a quoted referrer and a quoted user agent. Lines that do not match this shape are malformed: count them and skip them. Query strings (from `?` on) are not part of the path.

Print a report:
1. `requests N` (well-formed lines), `malformed N`, `bytes N` (total size), `unique_ips N`, each on its own line.
2. `status` then one line per status class that appears, in order (`2xx`, `3xx`, `4xx`, `5xx`), as `  2xx N`.
3. `top_paths` then the 3 most requested paths as `  N PATH`, by count descending, then path ascending.
4. `hours` then one line per hour (00-23, only hours with requests, ascending) as `  HH N BAR` where BAR is `#` repeated round(N * 20 / max) times (round half up, at least 1), max being the busiest hour's count.
5. `bots N`: requests whose user agent contains `bot` (any case).
6. `errors_by_ip` then, for IPs with at least 2 responses of status 400 or more, `  IP N`, by N descending then IP ascending (compare IPs as strings).
""", [
"\n".join([
'10.0.0.1 - - [10/Oct/2026:13:55:36 +0000] "GET /index.html HTTP/1.1" 200 2326 "-" "Mozilla/5.0"',
'10.0.0.2 - - [10/Oct/2026:13:57:01 +0000] "GET /about?x=1 HTTP/1.1" 200 512 "-" "Googlebot/2.1"',
'10.0.0.1 - - [10/Oct/2026:14:02:11 +0000] "POST /login HTTP/1.1" 401 - "-" "curl/8"',
'garbage line here',
'10.0.0.1 - - [10/Oct/2026:14:05:00 +0000] "POST /login HTTP/1.1" 500 17 "-" "curl/8"',
'10.0.0.3 - - [10/Oct/2026:09:00:00 +0000] "GET /index.html HTTP/1.1" 304 0 "http://x" "SomeBOT"',
'10.0.0.2 - - [10/Oct/2026:14:59:59 +0000] "GET /index.html HTTP/2" 404 99 "-" "Mozilla"',
'10.0.0.2 - - [10/Oct/2026:14:00:00 +0000] "GET /about HTTP/2" 404 1 "-" "Mozilla"',
]) + "\n",
"\n".join(f'192.168.1.{i % 4} - - [01/Jan/2026:{h:02d}:{i % 60:02d}:00 +0000] "GET /p{i % 5} HTTP/1.1" {[200, 200, 301, 404, 503][i % 5]} {i * 10} "-" "agent{i % 3}"'
          for i, h in enumerate([0, 0, 1, 5, 5, 5, 5, 5, 23, 23, 1, 0])) + '\n- - [x] "" 1 2\n',
])
def _(s):
    pat = re.compile(r'^(\S+) - - \[(\d{2})/(\w{3})/(\d{4}):(\d{2}):(\d{2}):(\d{2}) [+-]\d{4}\] "(\S+) (\S+) (\S+)" (\d{3}) (\d+|-) "([^"]*)" "([^"]*)"$')
    n = bad = byt = bots = 0; ips = set(); cls = {}; paths = {}; hours = {}; errs = {}
    for l in s.split("\n"):
        if not l: continue
        m = pat.match(l)
        if not m: bad += 1; continue
        ip, hh, meth, path, st, size, ua = m.group(1), m.group(5), m.group(8), m.group(9), int(m.group(11)), m.group(12), m.group(14)
        n += 1; byt += 0 if size == "-" else int(size); ips.add(ip)
        c = f"{st // 100}xx"; cls[c] = cls.get(c, 0) + 1
        p = path.split("?")[0]; paths[p] = paths.get(p, 0) + 1
        hours[hh] = hours.get(hh, 0) + 1
        if "bot" in ua.lower(): bots += 1
        if st >= 400: errs[ip] = errs.get(ip, 0) + 1
    o = [f"requests {n}", f"malformed {bad}", f"bytes {byt}", f"unique_ips {len(ips)}", "status"]
    o += [f"  {c} {cls[c]}" for c in ["2xx", "3xx", "4xx", "5xx"] if c in cls]
    o.append("top_paths")
    o += [f"  {c} {p}" for p, c in sorted(paths.items(), key=lambda kv: (-kv[1], kv[0]))[:3]]
    o.append("hours")
    mx = max(hours.values()) if hours else 1
    o += [f"  {h} {hours[h]} " + "#" * max(1, int(hours[h] * 20 / mx + 0.5)) for h in sorted(hours)]
    o.append(f"bots {bots}")
    o.append("errors_by_ip")
    o += [f"  {ip} {c}" for ip, c in sorted(errs.items(), key=lambda kv: (-kv[1], kv[0])) if c >= 2]
    return "\n".join(o) + "\n"

# ---------------------------------------------------------------- timetable
@task("timetable", """
Stdin has two sections separated by a line `---`.

The first section is a train timetable, one trip per line: `TRAIN STATION HH:MM STATION HH:MM ...`, the stations a train calls at with its departure time from each (the last time is its arrival). Times are within one day.

The second section has queries `FROM TO HH:MM`: a traveller at station FROM at that time wants to reach TO as early as possible. Changing trains at a station needs at least 5 minutes (arrival time + 5 <= departure time), except staying on the same train. A traveller can board a train at FROM at or after the query time.

For each query print `FROM TO HH:MM via T1>T2...` with the earliest arrival time and the trains used in order (if several routes arrive equally early, prefer the fewest trains, then the alphabetically smallest train list joined by `>`), or `FROM TO unreachable`. If FROM equals TO, print `FROM TO HH:MM via -` with the query time.
""", [
"R1 A 08:00 B 08:30 C 09:00\nR2 B 08:40 D 09:10\nR3 A 08:10 D 09:30\nR4 C 09:04 D 09:05\nR5 C 09:05 D 09:08\n---\nA D 07:00\nA C 08:00\nB D 08:31\nD A 08:00\nC C 12:00\nA D 08:05\n",
"X P 10:00 Q 10:20 R 10:40 S 11:00\nY Q 10:25 S 10:50\nZ Q 10:24 S 10:50\nW R 10:45 S 10:49\n---\nP S 09:00\nQ S 10:21\nP R 10:01\n",
])
def _(s):
    a, b = s.split("---\n")
    trips = []
    for l in a.strip().split("\n"):
        p = l.split(); stops = [(p[i], int(p[i + 1][:2]) * 60 + int(p[i + 1][3:])) for i in range(1, len(p), 2)]
        trips.append((p[0], stops))
    def fmt(t): return f"{t // 60:02d}:{t % 60:02d}"
    out = []
    for q in b.strip().split("\n"):
        f, to, tm = q.split(); t0 = int(tm[:2]) * 60 + int(tm[3:])
        if f == to: out.append(f"{f} {to} {tm} via -"); continue
        # Exhaustive search over trip sequences (small inputs): state = (station, time, trains, last train).
        best = None
        def dfs(st, t, used):
            nonlocal best
            if st == to:
                key = (t, len(used), ">".join(used))
                if best is None or key < best: best = key
                return
            for name, stops in trips:
                if name in used: continue
                for i, (sn, dt) in enumerate(stops[:-1]):
                    need = t if not used else t + 5
                    if sn == st and dt >= (t0 if not used else need):
                        for sn2, at in stops[i + 1:]:
                            dfs(sn2, at, used + [name])
        dfs(f, t0, [])
        out.append(f"{f} {to} unreachable" if best is None else f"{f} {to} {fmt(best[0])} via {best[2]}")
    return "".join(x + "\n" for x in out)

# ---------------------------------------------------------------- template
@task("template", """
Stdin has a JSON object on the first line, then a template (all remaining lines). Render the template with the data and print it.

- `{{name}}` inserts a value. Names can be dotted paths into objects: `{{user.city}}`. Strings insert as-is, numbers as JSON writes them (integers without a decimal point), `true`/`false`, and `null` or a missing value insert nothing. Arrays and objects insert as compact JSON.
- `{{name|upper}}`, `{{name|lower}}`, `{{name|len}}` (length of a string or array), `{{name|default:TEXT}}` (TEXT if the value is missing, null or an empty string). Filters can chain: `{{name|default:anon|upper}}`.
- `{{#each items}}...{{/each}}` repeats its body for each element of the array `items`. Inside, `{{.}}` is the element itself, names are looked up in the element first (if it is an object) and then outside, and `{{@index}}` is the 0-based index. Blocks can nest.
- `{{#if name}}...{{else}}...{{/if}}` renders the first part if the value is truthy (not missing, null, false, 0, empty string or empty array), otherwise the `{{else}}` part, which is optional. Blocks can nest.
- Everything else is copied unchanged, including newlines.
""", [
'{"user": {"name": "ann", "city": "Oslo"}, "items": [{"n": "pen", "q": 2}, {"n": "ink", "q": 0}], "tags": ["a", "b"], "vip": true, "note": ""}\nHello {{user.name|upper}} from {{user.city}}!\n{{#each items}}{{@index}}. {{n}}{{#if q}} x{{q}}{{else}} (out){{/if}}\n{{/each}}Tags: {{#each tags}}[{{.}}]{{/each}} ({{tags|len}})\n{{#if vip}}VIP{{/if}}{{#if note}}N{{else}}no note{{/if}} {{note|default:none}} {{missing|default:x|upper}}\n',
'{"a": [[1, 2], [3]], "o": {"k": null, "f": 1.5}, "z": 0}\n{{#each a}}<{{#each .}}{{.}}{{/each}}>{{/each}}\n{{o}} {{o.k}}|{{o.f}} {{#if z}}yes{{else}}{{z}}{{/if}} {{a|len}}\n',
])
def _(s):
    first, tpl = s.split("\n", 1); data = json.loads(first)
    def lookup(path, scopes):
        if path == ".": return scopes[-1][0]
        if path == "@index": return scopes[-1][1]
        parts = path.split(".")
        for sc, _ in reversed(scopes):
            if isinstance(sc, dict) and parts[0] in sc:
                v = sc
                for p in parts:
                    v = v.get(p) if isinstance(v, dict) else None
                return v
        return None
    def show(v):
        if v is None: return ""
        if isinstance(v, bool): return "true" if v else "false"
        if isinstance(v, str): return v
        return json.dumps(v, separators=(",", ":"))
    def truthy(v): return not (v is None or v is False or v == 0 and not isinstance(v, bool) or v == "" or v == [])
    toks = re.split(r"(\{\{.*?\}\})", tpl)
    def render(i, scopes, stop):
        out = []
        while i < len(toks):
            t = toks[i]
            if t.startswith("{{") and t.endswith("}}"):
                inner = t[2:-2].strip()
                if inner in stop: return "".join(out), i
                if inner.startswith("#each "):
                    arr = lookup(inner[6:].strip(), scopes); j = None
                    if isinstance(arr, list) and arr:
                        for k, el in enumerate(arr):
                            body, j = render(i + 1, scopes + [(el, k)], ("/each",)); out.append(body)
                    else:
                        _, j = render(i + 1, scopes, ("/each",))
                    i = j + 1; continue
                if inner.startswith("#if "):
                    v = truthy(lookup(inner[4:].strip(), scopes))
                    a, j = render(i + 1, scopes, ("else", "/if"))
                    if toks[j][2:-2].strip() == "else":
                        b, j = render(j + 1, scopes, ("/if",))
                    else: b = ""
                    out.append(a if v else b); i = j + 1; continue
                name, *filters = inner.split("|"); v = lookup(name.strip(), scopes)
                for f in filters:
                    if f == "upper": v = show(v).upper()
                    elif f == "lower": v = show(v).lower()
                    elif f == "len": v = len(v) if isinstance(v, (str, list)) else 0
                    elif f.startswith("default:"): v = f[8:] if v is None or v == "" else v
                out.append(show(v))
            else: out.append(t)
            i += 1
        return "".join(out), i
    return render(0, [(data, 0)], ())[0]

for name, (prompt, inputs, fn) in T.items():
    d = f"tasks/{name}"; os.makedirs(f"{d}/tests", exist_ok=True)
    open(f"{d}/prompt.md", "w").write(prompt + "\n")
    for i, inp in enumerate(inputs):
        open(f"{d}/tests/{i}.in", "w").write(inp)
        open(f"{d}/tests/{i}.out", "w").write(fn(inp))
print(len(T), "tasks")
