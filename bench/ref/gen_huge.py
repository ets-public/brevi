# Held-out "huge" tasks (written 2026-11-01 for Adam's "much more complex programs" ask; never used to tune Brevi).
# Each one is meant to take a model 300 to 800 lines of Python over several files, well past the 150 to 400
# lines of gen_big3-5.py, where Brevi's plain-Python acceptance drops off (11% of 150+ line programs on 1 Nov).
# Writes tasks/<name>/{prompt.md,files,tests/N.in,N.out}.
import os, sys
sys.setrecursionlimit(10000)
T = {}
FILES = {}
ROLES = {}
def task(name, prompt, inputs, files, roles):
    def deco(fn):
        T[name] = (prompt.strip(), inputs, fn)
        FILES[name] = files
        ROLES[name] = roles
        return fn
    return deco

MULTI = """
Write the program as {n} files in the folder you are given: {files}. `main` is the entry point; each file imports what it needs from the others. {roles}
"""

# ---------------------------------------------------------------- minilang
@task("minilang", r"""
Write an interpreter for a small programming language. The program to run is read from stdin. It is first checked in full; only if it has no syntax error does it run.

Lines and tokens
- One statement per line. Blank lines are ignored, `#` starts a comment that runs to the end of the line (outside strings), and indentation means nothing.
- Tokens: integers (`0`, `42`; no sign, a minus is an operator), strings in double quotes (no escapes, cannot span lines), names (`[A-Za-z_][A-Za-z0-9_]*`), and the symbols `== != <= >= < > = + - * / % ( ) [ ] ,`. Spaces separate tokens and are otherwise ignored. Any other character, or a string with no closing quote, is a syntax error.
- Keywords (not usable as names): `print if elif else end while for in func return break continue and or not true false none`.

Statements
- `print E1, E2, ...` (at least one expression) prints the values separated by single spaces.
- `NAME = E` sets a variable. `NAME[I] = E` sets an item of the list in NAME.
- `if E` ... optional `elif E` blocks ... optional `else` ... `end`.
- `while E` ... `end`. `for NAME in E` ... `end`. `break` and `continue` work on the innermost loop.
- `func NAME(P1, P2, ...)` ... `end` defines a function (zero or more parameter names). `return` or `return E` leaves it; a function that ends without returning a value returns `none`.
- A call on its own, `NAME(ARGS)`, is a statement too (its value is dropped).

Syntax errors (print `syntax error line N` and stop; nothing runs). N is the first line, from the top, on which one of these is found, except for a missing `end`, which is reported at the line that opened the innermost block left open at the end of the program:
- a line that is not one of the statements above, has extra tokens after one, or has a bad expression;
- `elif`, `else` or `end` with no block to belong to, `elif` or `else` after an `else`, or `elif`/`else` in a `while`, `for` or `func` block;
- `func` anywhere but at the top level (inside any block), a function name used twice, or a function named like a built-in;
- `return` outside a function; `break` or `continue` outside a loop (a loop outside the function does not count).

Expressions, from lowest to highest precedence:
1. `or`, then `and` (both left to right, and both stop early: `false and X` never evaluates X)
2. `not E`
3. one comparison at most: `== != < <= > >=`
4. `+ -` (left to right)
5. `* / %` (left to right)
6. unary minus
7. indexing `E[I]` and calls `NAME(ARGS)`; primaries are integers, strings, `true`, `false`, `none`, names, `( E )` and list literals `[E1, E2, ...]` (possibly empty).

Values and rules
- Values are integers, strings, booleans, `none`, and lists. Lists are shared, not copied: after `b = a`, a change made through `b` is seen through `a`.
- `if`, `while`, `and`, `or` and `not` need booleans, otherwise it is a type error.
- `+` adds integers and joins two strings or two lists (into a new list). `- * / %` need integers; `/` and `%` round toward minus infinity (`-7 / 2` is `-4`, `-7 % 2` is `1`); dividing by zero is an error `division by zero`.
- `==` and `!=` compare any two values: values of different types are never equal (`1 == true` is false); lists are equal when their items are. `< <= > >=` compare two integers or two strings (by character codes); anything else is a type error.
- Indexing works on a list or a string (giving a one-character string), with an integer index; a negative index counts from the end (`-1` is the last). Out of range is the error `index out of range`; anything else is a type error.
- Variables: at the top level, a name is a global variable. Inside a function, parameters and names assigned in it are local to that call; reading a name looks at the call's locals first, then the globals. Reading a name that is not set is the error `unknown name NAME`.
- `for NAME in E`: E must be a list (its items as they are when the loop starts) or a string (its characters); anything else is a type error. NAME is set like a variable on each round.
- Functions can be called before the line that defines them. Calling a name that is neither a function nor a built-in is the error `unknown function NAME`; the wrong number of arguments is the error `wrong number of arguments`.
- Built-ins: `len(X)` (list or string), `str(X)` (the value as `print` shows it), `int(X)` (an integer is returned as it is; a string of digits with an optional leading `-` becomes that integer; any other string is the error `bad number`; other values are a type error), `push(L, X)` (adds X to the end of list L, returns `none`), `pop(L)` (removes and returns the last item; an empty list is `index out of range`), `range(N)` (the list `[0, 1, ..., N-1]`, empty when N <= 0). Wrong argument types are type errors.
- Printing: integers in decimal; strings as they are; `true`, `false`, `none`; lists as `[A, B, C]` with `, ` between items, where strings inside lists are shown in double quotes: `[1, "a", [true]]`.

Runtime errors print `error line N: MESSAGE` (MESSAGE is one of `unknown name X`, `unknown function X`, `wrong number of arguments`, `type error`, `division by zero`, `index out of range`, `bad number`) and stop the program; what was printed before stays. N is the line of the statement that was running (inside a function, the statement in the function body, not the line of the call).
""", [
"""# basics
x = 3
y = x * 4 - 2
print x, y, "hi"
print 7 / 2, -7 / 2, 7 % 3, -7 % 2
print "ab" + "cd", [1, 2] + [3], len("hello"), len([1, [2, 3]])
print 1 == 1, 1 == true, "a" < "b", [1, [2]] == [1, [2]], none
s = "result: " + str(x + y)
print s
print str([1, "two", [true, none]])
""",
"""func fib(n)
  if n < 2
    return n
  end
  return fib(n - 1) + fib(n - 2)
end
i = 0
out = []
while i < 15
  push(out, fib(i))
  i = i + 1
end
print out
print fact(10)
func fact(n)
  r = 1
  for k in range(n)
    r = r * (k + 1)
  end
  return r
end
""",
"""a = [1, 2, 3]
b = a
push(b, 4)
print a, len(a)
b[0] = 99
print a[0], a[-1], a[-4]
func addall(xs, v)
  for i in range(len(xs))
    xs[i] = xs[i] + v
  end
end
addall(a, 10)
print a
print pop(a), a
c = a + [0]
push(c, 5)
print a, c
""",
"""# scope
g = 10
func f(x)
  g = x + 1
  return g + h
end
h = 5
print f(1), g
func show()
  print "g is", g
end
show()
n = 0
for ch in "hello"
  if ch == "l"
    continue
  elif ch == "o"
    break
  else
    n = n + 1
  end
end
print n
""",
"""func sort(xs)
  n = len(xs)
  i = 1
  while i < n
    j = i
    while j > 0 and xs[j - 1] > xs[j]
      t = xs[j]
      xs[j] = xs[j - 1]
      xs[j - 1] = t
      j = j - 1
    end
    i = i + 1
  end
  return xs
end
data = [5, 3, 9, 1, 5, 0, -2]
sort(data)
print data
words = ["pear", "apple", "fig"]
print sort(words)
print not (1 < 2) or 3 >= 3
""",
"""print "start"
x = 1
if x == 1
  print "one"
elif x == 2
  print "two"
end
func bad()
  print "in bad"
  return 1 / 0
end
print "before"
y = bad()
print "never"
""",
"""print "a"
while true
  print "b"
  x = 1
end
func f()
  break
end
""",
"""x = 1
if x > 0
  print "pos"
  while x < 3
    x = x + 1
""",
"""print "x"
func f(a)
  return a
end
print f(1, 2)
""",
"""xs = [1, 2]
print xs[2]
""",
"""print "ok"
print 1 + "a"
""",
"""print "fine"
y = 2 +
""",
"""print int("42") + 1, int("-7"), int(5)
print int("4x")
""",
"""print 1 < 2 and 2 < 3
if 1
  print "no"
end
""",
"""func len(x)
  return 0
end
""",
"""x = 5
print x
else
""",
"""print undefined_thing
""",
"""print nope(1)
""",
"""s = "say \"hi\""
""",
"""print "a" # comment with "quote
print [] == [], [] + [], range(0), range(3)
print "x"[0], "abc"[-1]
z = [[1, 2], [3]]
z[0][1] = 7
print z
""",
"""func outer()
  func inner()
  end
end
""",
"""func f()
  return
end
print f()
for i in 5
  print i
end
""",
"""func count(n)
  if n == 0
    return 0
  end
  return 1 + count(n - 1)
end
print count(50)
func collatz(n)
  steps = 0
  while n != 1
    if n % 2 == 0
      n = n / 2
    else
      n = 3 * n + 1
    end
    steps = steps + 1
  end
  return steps
end
best = 0
arg = 0
for k in range(30)
  if k > 0
    c = collatz(k)
    if c > best
      best = c
      arg = k
    end
  end
end
print arg, best
""",
"""x = 1
if x == 1
  print "a"
else
  print "b"
elif x == 2
  print "c"
end
""",
"""print -3 * -2, -(4 + 1), 10 - 2 - 3, 2 * 3 % 4, 100 / 7 / 2
print "b" >= "a", "abc" < "abd", [1] != [1, 2], none == none, false == false
print [1] < [2]
""",
], files=["main", "lexer", "parser", "interp"],
roles="`lexer` turns a line into tokens; `parser` checks the whole program and builds its statements and expressions; `interp` runs them (values, scopes, built-ins, errors); `main` reads stdin, calls the others and prints.")
def minilang(src):
    KW = set("print if elif else end while for in func return break continue and or not true false none".split())
    BUILTINS = {"len", "str", "int", "push", "pop", "range"}
    out = []

    class SyntaxErr(Exception):
        def __init__(self, n): self.n = n

    def lex(line, n):
        toks, i = [], 0
        while i < len(line):
            c = line[i]
            if c in " \t": i += 1
            elif c == "#": break
            elif c.isdigit():
                j = i
                while j < len(line) and line[j].isdigit(): j += 1
                toks.append(("int", int(line[i:j]))); i = j
            elif c == '"':
                j = line.find('"', i + 1)
                if j < 0: raise SyntaxErr(n)
                toks.append(("str", line[i + 1:j])); i = j + 1
            elif c.isalpha() or c == "_":
                j = i
                while j < len(line) and (line[j].isalnum() or line[j] == "_") and line[j].isascii(): j += 1
                w = line[i:j]
                toks.append(("kw", w) if w in KW else ("name", w)); i = j
            elif line[i:i + 2] in ("==", "!=", "<=", ">="):
                toks.append(("op", line[i:i + 2])); i += 2
            elif c in "<>=+-*/%()[],":
                toks.append(("op", c)); i += 1
            else:
                raise SyntaxErr(n)
        return toks

    class P:
        def __init__(self, toks, n): self.t, self.i, self.n = toks, 0, n
        def peek(self): return self.t[self.i] if self.i < len(self.t) else (None, None)
        def eat(self, kind, val=None):
            k, v = self.peek()
            if k == kind and (val is None or v == val):
                self.i += 1; return v
            raise SyntaxErr(self.n)
        def at(self, kind, val=None):
            k, v = self.peek(); return k == kind and (val is None or v == val)
        def done(self):
            if self.i != len(self.t): raise SyntaxErr(self.n)
        def expr(self):
            e = self.and_()
            while self.at("kw", "or"): self.i += 1; e = ("or", e, self.and_())
            return e
        def and_(self):
            e = self.not_()
            while self.at("kw", "and"): self.i += 1; e = ("and", e, self.not_())
            return e
        def not_(self):
            if self.at("kw", "not"): self.i += 1; return ("not", self.not_())
            return self.cmp()
        def cmp(self):
            e = self.add()
            k, v = self.peek()
            if k == "op" and v in ("==", "!=", "<", "<=", ">", ">="):
                self.i += 1; e = ("bin", v, e, self.add())
            return e
        def add(self):
            e = self.mul()
            while self.peek()[0] == "op" and self.peek()[1] in "+-": op = self.peek()[1]; self.i += 1; e = ("bin", op, e, self.mul())
            return e
        def mul(self):
            e = self.un()
            while self.peek()[0] == "op" and self.peek()[1] in ("*", "/", "%"): op = self.peek()[1]; self.i += 1; e = ("bin", op, e, self.un())
            return e
        def un(self):
            if self.at("op", "-"): self.i += 1; return ("neg", self.un())
            return self.post()
        def args(self, close):
            xs = []
            if self.at("op", close): self.i += 1; return xs
            while True:
                xs.append(self.expr())
                if self.at("op", ","): self.i += 1; continue
                self.eat("op", close); return xs
        def post(self):
            k, v = self.peek()
            if k is None: raise SyntaxErr(self.n)
            self.i += 1
            if k in ("int", "str"): e = ("lit", v)
            elif k == "kw" and v in ("true", "false", "none"): e = ("lit", {"true": True, "false": False, "none": None}[v])
            elif k == "name":
                if self.at("op", "("): self.i += 1; e = ("call", v, self.args(")"))
                else: e = ("var", v)
            elif k == "op" and v == "(":
                e = self.expr(); self.eat("op", ")")
            elif k == "op" and v == "[":
                e = ("list", self.args("]"))
            else: raise SyntaxErr(self.n)
            while self.at("op", "["):
                self.i += 1; ix = self.expr(); self.eat("op", "]"); e = ("index", e, ix)
            return e

    def parse(lines):
        funcs = {}
        root = []
        # stack of (kind, opener line, body list, node)
        stack = [("top", 0, root, None)]
        def in_func(): return any(s[0] == "func" for s in stack)
        def in_loop():
            # loops of the current function only (functions are only at the top, so the func is stack[1])
            return any(s[0] in ("while", "for") for s in stack)
        for n, line in enumerate(lines, 1):
            toks = lex(line, n)
            if not toks: continue
            p = P(toks, n)
            k, v = toks[0]
            cur = stack[-1]
            if k == "kw" and v == "print":
                p.i = 1; es = [p.expr()]
                while p.at("op", ","): p.i += 1; es.append(p.expr())
                p.done(); cur[2].append(("print", n, es))
            elif k == "kw" and v in ("if", "while"):
                p.i = 1; e = p.expr(); p.done()
                if v == "if":
                    node = ["if", n, [(e, [])], None]
                    cur[2].append(node); stack.append(("if", n, node[2][0][1], node))
                else:
                    body = []; cur[2].append(("while", n, e, body)); stack.append(("while", n, body, None))
            elif k == "kw" and v in ("elif", "else"):
                if cur[0] not in ("if", "else"): raise SyntaxErr(n)
                if cur[0] == "else": raise SyntaxErr(n)
                node = cur[3]
                p.i = 1
                if v == "elif":
                    e = p.expr(); p.done(); body = []; node[2].append((e, body)); stack[-1] = ("if", cur[1], body, node)
                else:
                    p.done(); body = []; node[3] = body; stack[-1] = ("else", cur[1], body, node)
            elif k == "kw" and v == "end":
                p.i = 1; p.done()
                if len(stack) == 1: raise SyntaxErr(n)
                stack.pop()
            elif k == "kw" and v == "for":
                p.i = 1; name = p.eat("name"); p.eat("kw", "in"); e = p.expr(); p.done()
                body = []; cur[2].append(("for", n, name, e, body)); stack.append(("for", n, body, None))
            elif k == "kw" and v == "func":
                p.i = 1; name = p.eat("name"); p.eat("op", "(")
                params = []
                if p.at("op", ")"): p.i += 1
                else:
                    while True:
                        params.append(p.eat("name"))
                        if p.at("op", ","): p.i += 1; continue
                        p.eat("op", ")"); break
                p.done()
                if len(stack) != 1 or name in funcs or name in BUILTINS: raise SyntaxErr(n)
                body = []; funcs[name] = (params, body); stack.append(("func", n, body, None))
            elif k == "kw" and v == "return":
                if not in_func(): raise SyntaxErr(n)
                p.i = 1; e = None if p.i == len(toks) else p.expr(); p.done()
                cur[2].append(("return", n, e))
            elif k == "kw" and v in ("break", "continue"):
                p.i = 1; p.done()
                if not in_loop(): raise SyntaxErr(n)
                cur[2].append((v, n))
            elif k == "name":
                if len(toks) > 1 and toks[1] == ("op", "="):
                    p.i = 2; e = p.expr(); p.done(); cur[2].append(("set", n, v, e))
                elif len(toks) > 1 and toks[1] == ("op", "("):
                    e = p.expr(); p.done()
                    if e[0] != "call": raise SyntaxErr(n)
                    cur[2].append(("expr", n, e))
                elif len(toks) > 1 and toks[1] == ("op", "["):
                    # NAME[I]... = E : parse the target as an index expression
                    p.i = 0
                    tgt = p.post()
                    if tgt[0] != "index" or not p.at("op", "="): raise SyntaxErr(n)
                    p.i += 1; e = p.expr(); p.done()
                    cur[2].append(("setitem", n, tgt[1], tgt[2], e))
                else: raise SyntaxErr(n)
            else:
                raise SyntaxErr(n)
        if len(stack) > 1: raise SyntaxErr(stack[-1][1])
        return root, funcs

    class RunErr(Exception):
        def __init__(self, msg): self.msg = msg
    class Ret(Exception):
        def __init__(self, v): self.v = v
    class Brk(Exception): pass
    class Cont(Exception): pass

    def typ(v):
        if isinstance(v, bool): return "bool"
        if v is None: return "none"
        if isinstance(v, int): return "int"
        if isinstance(v, str): return "str"
        return "list"
    def show(v, inner=False):
        t = typ(v)
        if t == "bool": return "true" if v else "false"
        if t == "none": return "none"
        if t == "str": return '"' + v + '"' if inner else v
        if t == "int": return str(v)
        return "[" + ", ".join(show(x, True) for x in v) + "]"
    def eq(a, b):
        if typ(a) != typ(b): return False
        if typ(a) == "list": return len(a) == len(b) and all(eq(x, y) for x, y in zip(a, b))
        return a == b
    def need(v, t):
        if typ(v) != t: raise RunErr("type error")
        return v
    def index(c, i):
        if typ(c) not in ("list", "str") or typ(i) != "int": raise RunErr("type error")
        if i < -len(c) or i >= len(c): raise RunErr("index out of range")
        return i

    try:
        root, funcs = parse(src.split("\n"))
    except SyntaxErr as e:
        return f"syntax error line {e.n}\n"

    G = {}
    def ev(e, L):
        k = e[0]
        if k == "lit": return e[1]
        if k == "var":
            if e[1] in L: return L[e[1]]
            if e[1] in G: return G[e[1]]
            raise RunErr(f"unknown name {e[1]}")
        if k == "list": return [ev(x, L) for x in e[1]]
        if k == "index":
            c = ev(e[1], L); i = ev(e[2], L); return c[index(c, i)]
        if k == "neg": return -need(ev(e[1], L), "int")
        if k == "not": return not need(ev(e[1], L), "bool")
        if k == "and": return need(ev(e[2], L), "bool") if need(ev(e[1], L), "bool") else False
        if k == "or": return True if need(ev(e[1], L), "bool") else need(ev(e[2], L), "bool")
        if k == "bin":
            op, a, b = e[1], ev(e[2], L), ev(e[3], L)
            if op == "==": return eq(a, b)
            if op == "!=": return not eq(a, b)
            if op in ("<", "<=", ">", ">="):
                if typ(a) != typ(b) or typ(a) not in ("int", "str"): raise RunErr("type error")
                return {"<": a < b, "<=": a <= b, ">": a > b, ">=": a >= b}[op]
            if op == "+":
                if typ(a) == typ(b) and typ(a) in ("int", "str", "list"): return a + b
                raise RunErr("type error")
            need(a, "int"); need(b, "int")
            if op == "-": return a - b
            if op == "*": return a * b
            if b == 0: raise RunErr("division by zero")
            return a // b if op == "/" else a % b
        if k == "call":
            name, args = e[1], [ev(x, L) for x in e[2]]
            if name in BUILTINS:
                want = 2 if name == "push" else 1
                if len(args) != want: raise RunErr("wrong number of arguments")
                x = args[0]
                if name == "len":
                    if typ(x) not in ("list", "str"): raise RunErr("type error")
                    return len(x)
                if name == "str": return show(x)
                if name == "int":
                    if typ(x) == "int": return x
                    need(x, "str")
                    d = x[1:] if x.startswith("-") else x
                    if not d or not all(c in "0123456789" for c in d): raise RunErr("bad number")
                    return int(x)
                if name == "push": need(x, "list").append(args[1]); return None
                if name == "pop":
                    need(x, "list")
                    if not x: raise RunErr("index out of range")
                    return x.pop()
                if name == "range": return list(range(need(x, "int")))
            if name not in funcs: raise RunErr(f"unknown function {name}")
            params, body = funcs[name]
            if len(params) != len(args): raise RunErr("wrong number of arguments")
            loc = dict(zip(params, args))
            try: run(body, loc)
            except Ret as r: return r.v
            return None
        raise AssertionError(k)

    def run(body, L):
        for s in body:
            k = s[0]
            try:
                if k == "print": out.append(" ".join(show(ev(x, L)) for x in s[2]))
                elif k == "set": L[s[2]] = ev(s[3], L)
                elif k == "setitem":
                    c = ev(s[2], L); i = ev(s[3], L)
                    if typ(c) != "list": raise RunErr("type error")
                    c[index(c, i)] = ev(s[4], L)
                elif k == "expr": ev(s[2], L)
                elif k == "if":
                    for cond, b in s[2]:
                        if need(ev(cond, L), "bool"): run(b, L); break
                    else:
                        if s[3] is not None: run(s[3], L)
                elif k == "while":
                    while need(ev(s[2], L), "bool"):
                        try: run(s[3], L)
                        except Brk: break
                        except Cont: pass
                elif k == "for":
                    it = ev(s[3], L)
                    if typ(it) not in ("list", "str"): raise RunErr("type error")
                    for x in list(it):
                        L[s[2]] = x
                        try: run(s[4], L)
                        except Brk: break
                        except Cont: pass
                elif k == "return": raise Ret(None if s[2] is None else ev(s[2], L))
                elif k == "break": raise Brk()
                elif k == "continue": raise Cont()
            except RunErr as e:
                if not hasattr(e, "line"): e.line = s[1]
                raise
    try:
        run(root, G)
    except RunErr as e:
        out.append(f"error line {e.line}: {e.msg}")
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- chess
@task("chess", r"""
Write a chess referee. The game starts from the standard position with white to move. Read lines from stdin, numbered from 1; blank lines are ignored (but counted).

Commands
- A move in coordinate form: from-square then to-square, like `e2e4` or `g8f6`, plus a fifth letter `q`, `r`, `b` or `n` for a pawn reaching the last rank (`e7e8q`). Squares are files `a`-`h` and ranks `1`-`8`, lower case. A line that is not of this form, and is not a command below, prints `line N: bad input`.
- `board` prints the board: eight lines for ranks 8 down to 1, each the rank number then the 8 squares from a to h, separated by single spaces, using `K Q R B N P` for white pieces, `k q r b n p` for black and `.` for empty; then the line `  a b c d e f g h` (two spaces first).
- `moves SQ` (SQ a square) prints `SQ: ` followed by the legal destination squares of the piece on SQ, sorted (`a1` < `a2` < ... < `h8`, comparing as text) and separated by spaces, or `SQ: none` when the square is empty, holds a piece of the side not to move, or that piece has no legal move. A pawn move that promotes is listed once.
- `undo` takes back the last move that was played (also after a game has ended, which reopens it), printing `undone XXXX` with that move as it was written; with no move to take back it prints `line N: nothing to undo`.

Move rules: the usual rules of chess, including castling (written as the king's move, `e1g1`; allowed when neither the king nor that rook has moved yet, the squares between them are empty, the king is not in check, and it neither passes through nor lands on a square the opponent attacks), en passant (only on the move right after the double step), promotion (a pawn moving to the last rank must name its new piece; a fifth letter on any other move makes it illegal) and the rule that a move may not leave the mover's own king attacked. A move that is not legal prints `line N: illegal move` and is ignored.

After a legal move:
- if the side now to move has no legal move: print `checkmate, white wins` (or black) when its king is attacked, otherwise `stalemate`. The game is then over: any further move prints `line N: game over` and is ignored (`board`, `moves` and `undo` still work; `moves` then always prints none).
- otherwise, if the king of the side now to move is attacked, print `check`.

At the end of input print `moves played: K` (moves on the board after undos), then the board as `board` prints it.
""", [
"""e2e4
e7e5
g1f3
b8c6
f1c4
g8f6
f3g5
d7d5
e4d5
f6d5
g5f7
e8f7
d1f3
board
moves f7
""",
"""f2f3
e7e5
g2g4
d8h4
e2e4
board
undo
undo
moves d8
g2g3
""",
"""e2e4
d7d5
e4e5
f7f5
moves e5
e5f6
board
e2e4
e1e3
e2e5x
hello
e2e4 extra
""",
"""e2e4
e7e5
g1f3
b8c6
f1c4
f8c5
moves e1
e1g1
moves e8
d7d6
board
""",
"""g1f3
g8f6
g2g3
g7g6
f1g2
f8g7
d2d3
d7d6
c1g5
e8g8
b1c3
h7h6
moves e1
e1c1
g5h6
a7a6
moves e1
d1d2
a6a5
moves e1
e1c1
board
""",
"""a2a4
b7b5
a4b5
a7a6
b5a6
c8b7
a6b7
h7h6
b7a8
b7a8q
board
moves a8
""",
"""e2e3
a7a5
d1h5
a8a6
h5a5
h7h5
h2h4
a6h6
a5c7
f7f6
c7d7
e8f7
d7b7
d8d3
b7b8
d3h7
b8c8
f7g6
c8e6
board
undo
undo
moves g6
""",
"""undo
e2e4
undo
undo
moves e2
d2d4
e7e6
moves f8
moves d4
""",
"""e2e4
e7e5
e1e2
e8e7
e2e1
board
moves e1
""",
"""d2d4
e7e5
d4e5
f7f5
e5f6
g8e7
f6g7
e7g6
moves g7
g7h8
g7h8n
board
""",
], files=["main", "board", "rules"],
roles="`board` holds the pieces, prints the board and parses squares; `rules` generates moves, finds attacks and checks legality (castling, en passant, promotion, check); `main` reads the commands, keeps the move history for undo and the game state, and prints.")
def chess(src):
    import re
    out = []
    start = ["RNBQKBNR", "PPPPPPPP", "........", "........", "........", "........", "pppppppp", "rnbqkbnr"]
    def sq(s): return (ord(s[0]) - 97, int(s[1]) - 1)
    def name(f, r): return "abcdefgh"[f] + str(r + 1)
    class S: pass
    st = S()
    st.b = [list(row) for row in start]  # b[rank][file]
    st.white = True
    st.moved = set()        # squares whose original piece has moved (for castling): king/rook home squares
    st.ep = None            # square a pawn can capture onto en passant
    hist = []
    def white_piece(p): return p.isupper()
    def own(p, w): return p != "." and white_piece(p) == w
    def attacked(b, f, r, by_white):
        # is square (f, r) attacked by side by_white?
        for df, dr in [(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)]:
            x, y = f + df, r + dr
            if 0 <= x < 8 and 0 <= y < 8 and b[y][x] == ("N" if by_white else "n"): return True
        for df in (-1, 0, 1):
            for dr in (-1, 0, 1):
                if df or dr:
                    x, y = f + df, r + dr
                    if 0 <= x < 8 and 0 <= y < 8 and b[y][x] == ("K" if by_white else "k"): return True
        pr = r - 1 if by_white else r + 1
        for df in (-1, 1):
            x = f + df
            if 0 <= x < 8 and 0 <= pr < 8 and b[pr][x] == ("P" if by_white else "p"): return True
        for dirs, pcs in (([(1, 0), (-1, 0), (0, 1), (0, -1)], "RQ"), ([(1, 1), (1, -1), (-1, 1), (-1, -1)], "BQ")):
            for df, dr in dirs:
                x, y = f + df, r + dr
                while 0 <= x < 8 and 0 <= y < 8:
                    p = b[y][x]
                    if p != ".":
                        if own(p, by_white) and p.upper() in pcs: return True
                        break
                    x += df; y += dr
        return False
    def king(b, w):
        for r in range(8):
            for f in range(8):
                if b[r][f] == ("K" if w else "k"): return f, r
    def pseudo(f, r):
        # (tf, tr, special) for the piece at f, r of the side to move
        b, w = st.b, st.white
        p = b[r][f].upper(); res = []
        def add(x, y, sp=None):
            if 0 <= x < 8 and 0 <= y < 8 and not own(b[y][x], w): res.append((x, y, sp))
        if p == "N":
            for df, dr in [(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)]: add(f + df, r + dr)
        elif p == "K":
            for df in (-1, 0, 1):
                for dr in (-1, 0, 1):
                    if df or dr: add(f + df, r + dr)
            home = 0 if w else 7
            if (f, r) == (4, home) and (4, home) not in st.moved and not attacked(b, 4, home, not w):
                if (7, home) not in st.moved and b[home][7] == ("R" if w else "r") and b[home][5] == b[home][6] == "." \
                        and not attacked(b, 5, home, not w) and not attacked(b, 6, home, not w):
                    res.append((6, home, "castle"))
                if (0, home) not in st.moved and b[home][0] == ("R" if w else "r") and b[home][1] == b[home][2] == b[home][3] == "." \
                        and not attacked(b, 3, home, not w) and not attacked(b, 2, home, not w):
                    res.append((2, home, "castle"))
        elif p == "P":
            d = 1 if w else -1
            if 0 <= r + d < 8 and b[r + d][f] == ".":
                res.append((f, r + d, None))
                if r == (1 if w else 6) and b[r + 2 * d][f] == ".": res.append((f, r + 2 * d, "double"))
            for df in (-1, 1):
                x, y = f + df, r + d
                if 0 <= x < 8 and 0 <= y < 8:
                    if b[y][x] != "." and not own(b[y][x], w): res.append((x, y, None))
                    elif st.ep == (x, y): res.append((x, y, "ep"))
        else:
            dirs = {"R": [(1, 0), (-1, 0), (0, 1), (0, -1)], "B": [(1, 1), (1, -1), (-1, 1), (-1, -1)]}
            ds = dirs.get(p) or dirs["R"] + dirs["B"]
            for df, dr in ds:
                x, y = f + df, r + dr
                while 0 <= x < 8 and 0 <= y < 8:
                    if b[y][x] == ".": res.append((x, y, None))
                    else:
                        if not own(b[y][x], w): res.append((x, y, None))
                        break
                    x += df; y += dr
        return res
    def apply(f, r, x, y, sp, promo):
        # returns undo record
        rec = ([row[:] for row in st.b], st.white, set(st.moved), st.ep)
        b = st.b; p = b[r][f]
        b[y][x] = p; b[r][f] = "."
        if sp == "ep": b[r][x] = "."
        if sp == "castle":
            if x == 6: b[y][5] = b[y][7]; b[y][7] = "."
            else: b[y][3] = b[y][0]; b[y][0] = "."
        if promo: b[y][x] = promo.upper() if st.white else promo
        st.moved |= {(f, r), (x, y)}
        st.ep = (f, (r + y) // 2) if sp == "double" else None
        st.white = not st.white
        return rec
    def restore(rec):
        st.b, st.white, st.moved, st.ep = [row[:] for row in rec[0]], rec[1], set(rec[2]), rec[3]
    def legal(f, r):
        res = []
        for x, y, sp in pseudo(f, r):
            rec = apply(f, r, x, y, sp, None)
            kf, kr = king(st.b, not st.white)
            ok = not attacked(st.b, kf, kr, st.white)
            restore(rec)
            if ok: res.append((x, y, sp))
        return res
    def any_legal():
        for r in range(8):
            for f in range(8):
                if own(st.b[r][f], st.white) and legal(f, r): return True
        return False
    def board():
        for r in range(7, -1, -1): out.append(str(r + 1) + " " + " ".join(st.b[r]))
        out.append("  a b c d e f g h")
    over = False
    for n, line in enumerate(src.split("\n"), 1):
        if n == len(src.split("\n")) and line == "": break
        if line.strip() == "": continue
        if line == "board": board(); continue
        m = re.fullmatch(r"moves ([a-h][1-8])", line)
        if m:
            f, r = sq(m.group(1))
            ds = [] if over or not own(st.b[r][f], st.white) else sorted({name(x, y) for x, y, _ in legal(f, r)})
            out.append(f"{m.group(1)}: " + (" ".join(ds) if ds else "none")); continue
        if line == "undo":
            if not hist: out.append(f"line {n}: nothing to undo"); continue
            rec, mv = hist.pop(); restore(rec); over = False
            out.append(f"undone {mv}"); continue
        m = re.fullmatch(r"([a-h][1-8])([a-h][1-8])([qrbn]?)", line)
        if not m: out.append(f"line {n}: bad input"); continue
        if over: out.append(f"line {n}: game over"); continue
        (f, r), (x, y), promo = sq(m.group(1)), sq(m.group(2)), m.group(3)
        mv = None
        if own(st.b[r][f], st.white):
            for tx, ty, sp in legal(f, r):
                if (tx, ty) == (x, y): mv = sp; break
            else: mv = False
        else: mv = False
        if mv is False: out.append(f"line {n}: illegal move"); continue
        promotes = st.b[r][f].upper() == "P" and y in (0, 7)
        if promotes != bool(promo): out.append(f"line {n}: illegal move"); continue
        rec = apply(f, r, x, y, mv, promo)
        hist.append((rec, line))
        kf, kr = king(st.b, st.white)
        chk = attacked(st.b, kf, kr, not st.white)
        if not any_legal():
            over = True
            out.append(("checkmate, " + ("black" if st.white else "white") + " wins") if chk else "stalemate")
        elif chk: out.append("check")
    out.append(f"moves played: {len(hist)}")
    board()
    return "\n".join(out) + "\n"

# ---------------------------------------------------------------- adventure
@task("adventure", r"""
Write a text adventure engine. Stdin holds a world description, then a line `---`, then the player's commands. Lines are numbered from 1 over the whole input; blank lines are ignored everywhere (but counted).

World lines (words are separated by single spaces; IDs are words of letters, digits and `_`):
- `room ID NAME...` a room with a name of one or more words.
- `exit FROM DIR TO` or `exit FROM DIR TO locked KEY`: a one-way exit from room FROM in direction DIR (`north`, `south`, `east`, `west`, `up`, `down`) to room TO, optionally locked, needing item KEY to unlock.
- `item ID WHERE WEIGHT NAME...` an item that starts in room WHERE, or is carried when WHERE is `player`; WEIGHT is a whole number from 0; the name is one or more words.
- `start ROOM` (exactly one), `capacity N` (at most one; a whole number; 10 when absent), `goal ITEM ROOM` (any number): the game is won when every goal item lies in its goal room.

The world is checked first, and every problem is printed, in line order (several messages for one line in the order listed here):
- `world line N: bad line` for a line of none of these forms (wrong word count, bad number, unknown direction, bad ID, unknown first word); such a line is otherwise ignored.
- `world line N: duplicate X` for a second room or item with ID X, a second exit from the same room in the same direction (X is `exit`), a second `start` or a second `capacity` (X is `start` or `capacity`); the later line is ignored.
- `world line N: unknown room X` / `world line N: unknown item X` for a room or item that no line of the world defines (rooms and items may be used before the line that defines them). Room and item IDs are separate: a room and an item may share an ID.
- after all lines, `world: no start` when no `start` line was accepted (one naming an unknown room still counts as accepted; that room is reported as unknown above), and `world: too heavy` when the carried items weigh more than the capacity.
If anything was printed, the game does not run and nothing more is printed. A world with no `---` line is all world.

Commands (anything else, including wrong word counts and unknown directions, prints `line N: what?`):
- `look`: print the room's name; then `items: ` and the IDs of the items in the room, sorted, joined by `, `, or `items: none`; then `exits: ` and the directions with an exit, in the order north, south, east, west, up, down, joined by `, `, each followed by ` (locked)` when locked, or `exits: none`.
- `go DIR`: with no exit that way print `you can't go that way`; if it is locked print `the way DIR is locked`; otherwise move there and print the new room's name.
- `take ITEM`: if the item is not in this room print `no ITEM here`; if carrying it would bring the carried weight over the capacity print `ITEM is too heavy`; otherwise print `taken ITEM`.
- `drop ITEM`: if not carried print `you don't have ITEM`; otherwise leave it in this room and print `dropped ITEM`.
- `inv`: `carrying: ` then the carried item IDs sorted and joined by `, ` (or `nothing`), then ` (W/C)` with the carried weight and the capacity.
- `unlock DIR`: with no exit that way print `you can't go that way`; if not locked print `it isn't locked`; if the key is not carried print `you need KEY`; otherwise print `unlocked`: the exit is unlocked, and so is the exit back (from the room it leads to, in the opposite direction, if it leads back to this room and is locked with the same key).
- `examine ITEM`: if the item is carried or in this room, print its name and weight as `NAME (W)`; otherwise `no ITEM here`.
- `score`: `visited V of R rooms in M moves`: V counts rooms the player has been in (the start room counts), R all rooms, M the successful `go` commands.

After any command, if there is at least one goal and every goal item lies in its goal room (on the floor, not carried), print `you win in M moves` and ignore the rest of the input. If the input ends without a win, print `game over after M moves`. A world whose goals are all met from the start is won as soon as the first command has run.
""", [
"""room hall Great Hall
room kitchen Old Kitchen
room cellar Damp Cellar
exit hall east kitchen
exit kitchen west hall
exit kitchen down cellar locked brass_key
exit cellar up kitchen locked brass_key
item brass_key hall 1 small brass key
item barrel kitchen 30 oak barrel
item lamp player 2 oil lamp
item gold cellar 5 bag of gold
start hall
capacity 8
goal gold hall
---
look
inv
take brass_key
go east
look
take barrel
go down
unlock down
go down
take gold
inv
go up
look
go west
score
drop gold
look
""",
"""room a Room A
room b Room B
exit a north b
start a
---
go north
go north
go south
fly
go sideways
take nothing
drop lamp
score
examine b
""",
"""room a Room A
room a Again
exit a north c
exit a north a
item key a x heavy key
item key2 nowhere 1 key two
item key3 a 1
start b
start a
capacity 5
capacity 6
goal ghost a
door a b
exit a west a locked mystery
""",
"""room only Lonely Room
item rock player 50 big rock
""",
"""room start Start Room
room end End Room
exit start east end
exit end west start
item flag start 1 red flag
item stone start 4 grey stone
item feather end 0 white feather
start start
capacity 4
goal flag end
goal feather start
---
examine stone
take stone
take flag
inv
drop stone
take flag
examine flag
go east
take feather
drop flag
look
go west
drop feather
look
""",
"""room x The X
item token x 1 a token
start x
goal token x
---
look
look
""",
"""room r1 First
room r2 Second
room r3 Third
exit r1 east r2 locked k
exit r2 west r1 locked k
exit r2 east r3 locked k2
item k r1 1 iron key
item k2 r3 1 steel key
start r1
---
unlock west
unlock east
take k
unlock east
unlock east
go east
look
unlock west
unlock east
go east
score
""",
"""
room  bad Bad
room good Good Room
item it good -1 thing
item i2 good 1
exit good up
start good extra
capacity 0
---
look
""",
"""room cave Big Cave
item cave player 1 cave map
item ring cave 0 gold ring
exit cave down cave
start cave
capacity 1
---
inv
take ring
take cave
drop cave
take cave
examine cave
inv
go down
score
""",
], files=["main", "world", "game"],
roles="`world` reads and checks the world lines and holds rooms, exits and items; `game` runs the commands against the state (location, carried items, locks, moves, goals); `main` splits stdin at `---`, calls them and prints.")
def adventure(src):
    import re
    out = []
    lines = src.split("\n")
    if lines and lines[-1] == "": lines.pop()
    sep = next((i for i, l in enumerate(lines) if l == "---"), len(lines))
    DIRS = ["north", "south", "east", "west", "up", "down"]
    OPP = {"north": "south", "south": "north", "east": "west", "west": "east", "up": "down", "down": "up"}
    ID = re.compile(r"[A-Za-z0-9_]+")
    isid = lambda w: bool(ID.fullmatch(w))
    isnum = lambda w: bool(re.fullmatch(r"\d+", w))
    rooms, items, exits = {}, {}, {}
    start, cap = None, None
    goals = []
    refs = []  # (line, kind, id)
    errs = []  # (line, order, msg)
    for i in range(sep):
        n, l = i + 1, lines[i]
        if l.strip() == "": continue
        w = l.split(" ")
        k = w[0]
        lerr = []
        if k == "room" and len(w) >= 3 and isid(w[1]) and all(w[2:]):
            if w[1] in rooms: lerr.append(f"duplicate {w[1]}")
            else: rooms[w[1]] = " ".join(w[2:])
        elif k == "exit" and (len(w) == 4 or (len(w) == 6 and w[4] == "locked")) and isid(w[1]) and isid(w[3]) and w[2] in DIRS and (len(w) == 4 or isid(w[5])):
            key = w[5] if len(w) == 6 else None
            if (w[1], w[2]) in exits: lerr.append("duplicate exit")
            else:
                exits[(w[1], w[2])] = [w[3], key]
            refs.append((n, "room", w[1])); refs.append((n, "room", w[3]))
            if key: refs.append((n, "item", key))
        elif k == "item" and len(w) >= 5 and isid(w[1]) and isid(w[2]) and isnum(w[3]) and all(w[4:]):
            if w[1] in items: lerr.append(f"duplicate {w[1]}")
            else: items[w[1]] = {"where": w[2], "w": int(w[3]), "name": " ".join(w[4:])}
            if w[2] != "player": refs.append((n, "room", w[2]))
        elif k == "start" and len(w) == 2 and isid(w[1]):
            if start is not None: lerr.append("duplicate start")
            else: start = w[1]
            refs.append((n, "room", w[1]))
        elif k == "capacity" and len(w) == 2 and isnum(w[1]):
            if cap is not None: lerr.append("duplicate capacity")
            else: cap = int(w[1])
        elif k == "goal" and len(w) == 3 and isid(w[1]) and isid(w[2]):
            goals.append((w[1], w[2])); refs.append((n, "item", w[1])); refs.append((n, "room", w[2]))
        else:
            lerr.append("bad line")
        for m in lerr: errs.append((n, m))
    # duplicates were appended in line order; add unknown references in their line's slot
    allm = [(n, 0, j, m) for j, (n, m) in enumerate(errs)]
    for j, (n, kind, x) in enumerate(refs):
        if (kind == "room" and x not in rooms) or (kind == "item" and x not in items):
            allm.append((n, 1, j, f"unknown {kind} {x}"))
    allm.sort()
    for n, _, _, m in allm: out.append(f"world line {n}: {m}")
    if cap is None: cap = 10
    if start is None or start not in rooms:
        if start is None: out.append("world: no start")
    if sum(it["w"] for it in items.values() if it["where"] == "player") > cap: out.append("world: too heavy")
    if out: return "\n".join(out) + "\n"
    here, moves, visited = start, 0, {start}
    carried = lambda: sorted(k for k, it in items.items() if it["where"] == "player")
    weight = lambda: sum(items[k]["w"] for k in carried())
    won = lambda: goals and all(items[i]["where"] == r for i, r in goals)
    for i in range(sep + 1, len(lines)):
        n, l = i + 1, lines[i]
        if l.strip() == "": continue
        w = l.split(" ")
        c = w[0]
        if l == "look":
            out.append(rooms[here])
            its = sorted(k for k, it in items.items() if it["where"] == here)
            out.append("items: " + (", ".join(its) if its else "none"))
            ex = [d + (" (locked)" if exits[(here, d)][1] else "") for d in DIRS if (here, d) in exits]
            out.append("exits: " + (", ".join(ex) if ex else "none"))
        elif c == "go" and len(w) == 2 and w[1] in DIRS:
            e = exits.get((here, w[1]))
            if not e: out.append("you can't go that way")
            elif e[1]: out.append(f"the way {w[1]} is locked")
            else:
                here = e[0]; moves += 1; visited.add(here); out.append(rooms[here])
        elif c == "take" and len(w) == 2:
            it = items.get(w[1])
            if not it or it["where"] != here: out.append(f"no {w[1]} here")
            elif weight() + it["w"] > cap: out.append(f"{w[1]} is too heavy")
            else: it["where"] = "player"; out.append(f"taken {w[1]}")
        elif c == "drop" and len(w) == 2:
            it = items.get(w[1])
            if not it or it["where"] != "player": out.append(f"you don't have {w[1]}")
            else: it["where"] = here; out.append(f"dropped {w[1]}")
        elif l == "inv":
            cs = carried()
            out.append("carrying: " + (", ".join(cs) if cs else "nothing") + f" ({weight()}/{cap})")
        elif c == "unlock" and len(w) == 2 and w[1] in DIRS:
            e = exits.get((here, w[1]))
            if not e: out.append("you can't go that way")
            elif not e[1]: out.append("it isn't locked")
            elif items[e[1]]["where"] != "player": out.append(f"you need {e[1]}")
            else:
                key = e[1]; e[1] = None
                back = exits.get((e[0], OPP[w[1]]))
                if back and back[0] == here and back[1] == key: back[1] = None
                out.append("unlocked")
        elif c == "examine" and len(w) == 2:
            it = items.get(w[1])
            if not it or it["where"] not in ("player", here): out.append(f"no {w[1]} here")
            else: out.append(f"{it['name']} ({it['w']})")
        elif l == "score":
            out.append(f"visited {len(visited)} of {len(rooms)} rooms in {moves} moves")
        else:
            out.append(f"line {n}: what?")
        if won():
            out.append(f"you win in {moves} moves")
            return "\n".join(out) + "\n"
    out.append(f"game over after {moves} moves")
    return "\n".join(out) + "\n"


for name, (prompt, inputs, fn) in T.items():
    d = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tasks", name)
    os.makedirs(f"{d}/tests", exist_ok=True)
    fs = FILES[name]
    prompt += "\n" + MULTI.format(n=len(fs), files=", ".join(f"`{f}`" for f in fs), roles=ROLES[name]).rstrip()
    open(f"{d}/files", "w").write(" ".join(fs) + "\n")
    open(f"{d}/prompt.md", "w").write(prompt + "\n")
    for i, inp in enumerate(inputs):
        open(f"{d}/tests/{i}.in", "w").write(inp)
        open(f"{d}/tests/{i}.out", "w").write(fn(inp))
print(len(T), "tasks")
