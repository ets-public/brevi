"""Fuzz Brevi's builds for panics, signals and wrong answers, with generated plain Python.

A seeded generator writes small programs in the style agents write (ints, floats, strings, lists, dicts, sets,
tuples, functions, classes, loops, aliased lists/dicts (1 case in 4), comprehensions, slicing, try/except around operations that raise). Each program
is a batch of independent cases, `def case_N(): ...`, each called under its own try/except so one raising case does
not hide the others. Every batch runs under CPython first (a batch CPython rejects or times out on is discarded),
then goes through `brevi build` twice, the normal path and BREVI_DYN=1, and each binary runs. `brevi run` is never
used: it falls back to CPython.

Outcomes, worst first:
  panic    the binary printed "panicked at" (a Rust panic: a P0 bug)
  signal   the binary died on a signal (a security bug until triaged; see SECURITY.md)
  wrong    stdout or the exit status differs from CPython's
  build    `brevi build` itself crashed (a panic or signal in the compiler)
  timeout  the binary ran past the timeout where CPython did not
A batch that fails is cut down to the single case that fails (each case is rebuilt alone), and that case is
written to the output folder (default bench/fuzz/found/) with the seed that made it, so it reproduces.
A refused build ("cannot build this") is fine: refusing is the right answer for Python Brevi does not handle.

    python3 tools/fuzz.py                       # 20 batches of 12 cases from seed 1
    python3 tools/fuzz.py --batches 200 --seed 7 --jobs 4
    python3 tools/fuzz.py --alias-only --cases 1  # only aliasing cases, one build each (see below)
    python3 tools/fuzz.py --show 7:3            # print batch 3 of seed 7 (the program, not run)
    python3 tools/fuzz.py --replay bench/fuzz/corpus   # rerun saved cases (CI runs the corpus on every PR)
    python3 tools/fuzz.py --fail-on panic,signal,build # exit 1 only on these (default: panic,signal,build)

--alias-only: when Brevi detects two names sharing a list it falls the WHOLE program back to the dynamic path, so in a
batch of 12 one detected case hides the typed build for the other 11. Use --cases 1 to find the cases it misses.

The corpus (bench/fuzz/corpus/) holds cases that once failed and were fixed: each must now match CPython, so a
fixed panic cannot come back. Env: BREVI (default target/release/brevi), PYTHON (default python3).
"""
import argparse, concurrent.futures as cf, json, os, random, re, shutil, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BREVI = os.environ.get("BREVI", str(ROOT / "target" / "release" / "brevi"))
PYTHON = os.environ.get("PYTHON", "python3")
TIMEOUT = 20

# ---------------------------------------------------------------- generator

INT, FLOAT, STR, BOOL, LINT, LSTR, DICT, TUP, SETI = "int", "float", "str", "bool", "list[int]", "list[str]", "dict", "tuple", "set"
TYPES = [INT, INT, INT, FLOAT, STR, STR, BOOL, LINT, LINT, LSTR, DICT, DICT, TUP, SETI]
WORDS = ["a", "b", "ab", "xy", "hello", "Brevi", "  pad ", "a,b,c", "x-y-z", "", "Zed", "42", "3.5", "üñí", "tab\tbed"]


class Gen:
    def __init__(self, rng):
        self.r = rng
        self.vars = {}  # name -> type
        self.n = 0
        self.depth = 0
        self.in_f = False  # inside an f-string: no nested f-string (PEP 701 same-quote nesting is 3.12+ only)

    def fstr(self, parts):
        if self.in_f:
            return self.lit(STR)
        self.in_f = True
        try:
            return parts()
        finally:
            self.in_f = False

    def fresh(self, t):
        self.n += 1
        name = f"v{self.n}"
        return name

    def pick(self, t):
        names = [k for k, v in self.vars.items() if v == t]
        return self.r.choice(names) if names else None

    # ---- expressions of a given type (never raising unless risky=True)
    def lit(self, t):
        r = self.r
        if t == INT:
            return str(r.choice([0, 1, 2, 3, -1, -7, 10, 255, 1000, 2**31, -(2**40), 10**20, r.randint(-50, 50)]))
        if t == FLOAT:
            return r.choice(["0.0", "1.5", "-2.25", "0.1", "3.0", "1e10", "-0.5", "2.675", "1e-5", "123.456"])
        if t == STR:
            return repr(r.choice(WORDS))
        if t == BOOL:
            return r.choice(["True", "False"])
        if t == LINT:
            return "[" + ", ".join(self.lit(INT) for _ in range(r.randint(0, 5))) + "]"
        if t == LSTR:
            return "[" + ", ".join(self.lit(STR) for _ in range(r.randint(0, 4))) + "]"
        if t == DICT:
            ks = r.sample(["a", "b", "c", "k", "zz", "x1"], r.randint(0, 4))
            return "{" + ", ".join(f"{k!r}: {self.lit(INT)}" for k in ks) + "}"
        if t == TUP:
            return f"({self.lit(INT)}, {self.lit(STR)})"
        if t == SETI:
            return "{" + ", ".join(self.lit(INT) for _ in range(r.randint(1, 4))) + "}" if r.random() < 0.8 else "set()"
        raise ValueError(t)

    def expr(self, t, d=0):
        x = self.expr1(t, d)
        return x if x.isidentifier() or x[:1] in "([{'\"" and self._closed(x) else f"({x})"

    @staticmethod
    def _closed(x):
        # true when x is one bracketed or quoted unit (so it needs no parentheses), e.g. "[1, 2]" but not "[1] + [2]"
        if x[0] in "'\"":
            try:
                import ast
                return isinstance(ast.parse(x, mode="eval").body, ast.Constant)
            except SyntaxError:
                return False
        pairs, depth = {"(": ")", "[": "]", "{": "}"}, 0
        for k, ch in enumerate(x):
            if ch in pairs:
                depth += 1
            elif ch in ")]}":
                depth -= 1
                if depth == 0 and k != len(x) - 1:
                    return False
        return True

    def expr1(self, t, d=0):
        r = self.r
        v = self.pick(t)
        if d > 2 or r.random() < 0.25:
            return v if v and r.random() < 0.7 else self.lit(t)
        e = lambda tt: self.expr(tt, d + 1)
        if t == INT:
            return r.choice([
                lambda: f"({e(INT)} {r.choice(['+', '-', '*'])} {e(INT)})",
                lambda: f"({e(INT)} // {r.choice(['3', '-2', '7'])})",
                lambda: f"({e(INT)} % {r.choice(['3', '-5', '10'])})",
                lambda: f"len({e(r.choice([STR, LINT, LSTR, DICT, SETI]))})",
                lambda: f"abs({e(INT)})",
                lambda: f"int({e(FLOAT)})",
                lambda: f"({e(INT)} {r.choice(['&', '|', '^'])} {e(INT)})",
                lambda: f"({e(INT)} << {r.randint(0, 70)})",
                lambda: f"({e(INT)} >> {r.randint(0, 5)})",
                lambda: f"sum({e(LINT)})",
                lambda: f"max({e(INT)}, {e(INT)})",
                lambda: f"min({e(LINT)}, default={e(INT)})",
                lambda: f"{e(STR)}.count({self.lit(STR)})",
                lambda: f"{e(STR)}.find({self.lit(STR)})",
                lambda: f"{e(DICT)}.get({self.lit(STR)}, {e(INT)})",
                lambda: f"({e(INT)} if {e(BOOL)} else {e(INT)})",
                lambda: f"round({e(FLOAT)})",
                lambda: f"{e(TUP)}[0]",
                lambda: f"hash({e(INT)}) % 1000",
                lambda: f"pow({e(INT)}, {r.randint(0, 9)})",
                lambda: f"({e(INT)} ** {r.randint(0, 6)})",
                lambda: f"int({r.choice(['str', 'repr'])}({e(INT)}))",
                lambda: f"ord({self.lit(STR)}[0] if {self.lit(STR)} else 'q')",
            ])()
        if t == FLOAT:
            return r.choice([
                lambda: f"({e(FLOAT)} {r.choice(['+', '-', '*'])} {e(FLOAT)})",
                lambda: f"({e(INT)} / {r.choice(['3', '-4', '7', '0.5'])})",
                lambda: f"float({e(INT)})",
                lambda: f"round({e(FLOAT)}, {r.randint(0, 3)})",
                lambda: f"abs({e(FLOAT)})",
                lambda: f"({e(FLOAT)} % {r.choice(['2', '-1.5', '0.3'])})",
                lambda: f"({e(FLOAT)} // {r.choice(['2', '-1.5'])})",
                lambda: f"max({e(FLOAT)}, {e(INT)})",
                lambda: f"sum([{e(FLOAT)}, {e(FLOAT)}])",
                lambda: f"(abs({e(FLOAT)}) ** {r.choice(['2', '0.5', '-1'])}) if {e(FLOAT)} > 0 else 0.0",  # no complex results: Brevi has no complex
            ])()
        if t == STR:
            return r.choice([
                lambda: f"({e(STR)} + {e(STR)})",
                lambda: f"str({e(r.choice([INT, FLOAT, BOOL, LINT, LSTR, DICT, TUP]))})",
                lambda: f"repr({e(r.choice([STR, FLOAT, LSTR, TUP]))})",
                lambda: f"{e(STR)}.{r.choice(['upper', 'lower', 'strip', 'title', 'swapcase', 'capitalize', 'lstrip', 'casefold'])}()",
                lambda: f"{e(STR)}.replace({self.lit(STR)}, {self.lit(STR)})",
                lambda: f"{e(STR)}[{r.choice(['1:', ':2', '::-1', '1:-1', '::2', '-3:'])}]",
                lambda: f"{self.lit(STR)}.join({e(LSTR)})",
                lambda: self.fstr(lambda: f'f"{{ {e(INT)} }}-{{ {e(STR)}!r}}-{{ {e(FLOAT)}:.2f}}"'),
                lambda: self.fstr(lambda: f'f"{{ {e(INT)}:>6}}|{{ {e(STR)}:<4}}|{{ {e(INT)}:x}}"'),
                lambda: f"({e(STR)} * {r.randint(0, 3)})",
                lambda: f"{e(STR)}.center({r.randint(0, 9)}, '*')",
                lambda: f"{e(STR)}.zfill({r.randint(0, 6)})",
                lambda: f"'%d:%s:%.3f' % ({e(INT)}, {e(STR)}, {e(FLOAT)})",
                lambda: f"chr(65 + abs({e(INT)}) % 26)",
                lambda: f"format({e(INT)}, ',')",
                lambda: f"{e(TUP)}[1]",
                lambda: f"max({e(LSTR)}, default='')",
                lambda: f"bin({e(INT)})",
            ])()
        if t == BOOL:
            return r.choice([
                lambda: f"({e(INT)} {r.choice(['<', '<=', '==', '!=', '>', '>='])} {e(INT)})",
                lambda: f"({e(STR)} {r.choice(['<', '==', 'in'])} {e(STR)})",
                lambda: f"({e(INT)} in {e(r.choice([LINT, SETI]))})",
                lambda: f"({self.lit(STR)} in {e(DICT)})",
                lambda: f"(not {e(BOOL)})",
                lambda: f"({e(BOOL)} {r.choice(['and', 'or'])} {e(BOOL)})",
                lambda: f"bool({e(r.choice([INT, STR, LINT, DICT]))})",
                lambda: f"{e(STR)}.{r.choice(['isdigit', 'isalpha', 'isspace', 'isupper', 'isalnum'])}()",
                lambda: f"{e(STR)}.startswith({self.lit(STR)})",
                lambda: f"any(x > {e(INT)} for x in {e(LINT)})",
                lambda: f"all(x for x in {e(LSTR)})",
                lambda: f"({e(FLOAT)} == {e(FLOAT)})",
                lambda: f"({e(LINT)} == {e(LINT)})",
                lambda: f"isinstance({e(r.choice([INT, STR, FLOAT]))}, {r.choice(['int', 'str', 'float', '(int, float)'])})",
            ])()
        if t == LINT:
            return r.choice([
                lambda: f"({e(LINT)} + {e(LINT)})",
                lambda: f"sorted({e(LINT)})",
                lambda: f"sorted({e(LINT)}, reverse=True)",
                lambda: f"sorted({e(LINT)}, key=lambda x: (x % 3, -x))",
                lambda: f"[x * {e(INT)} for x in {e(LINT)} if x % 2 == 0]",
                lambda: f"list(range({r.randint(-3, 3)}, {r.randint(0, 12)}, {r.choice([1, 2, 3, -1])}))",
                lambda: f"{e(LINT)}[{r.choice(['1:', ':2', '::-1', '1:-1', '::2', '-3:'])}]",
                lambda: f"[len(s) for s in {e(LSTR)}]",
                lambda: f"list({e(DICT)}.values())",
                lambda: f"sorted({e(SETI)})",
                lambda: f"list(map(abs, {e(LINT)}))",
                lambda: f"[a + b for a, b in zip({e(LINT)}, {e(LINT)})]",
                lambda: f"[i for i, _ in enumerate({e(LSTR)})]",
                lambda: f"({e(LINT)} * {r.randint(0, 3)})",
                lambda: f"list(reversed({e(LINT)}))",
                lambda: f"[x for x in {e(LINT)} for _ in range(2)]",
            ])()
        if t == LSTR:
            return r.choice([
                lambda: f"{e(STR)}.split({r.choice(['', repr(','), repr('-'), repr(' ')])})",
                lambda: f"sorted({e(LSTR)})",
                lambda: f"sorted({e(DICT)})",
                lambda: f"[s.upper() for s in {e(LSTR)}]",
                lambda: f"[str(x) for x in {e(LINT)}]",
                lambda: f"list({e(STR)})",
                lambda: f"({e(LSTR)} + {e(LSTR)})",
                lambda: f"list({e(DICT)}.keys())",
                lambda: f"sorted(set({e(LSTR)}))",
                lambda: f"{e(LSTR)}[::-1]",
            ])()
        if t == DICT:
            return r.choice([
                lambda: f"{{s: len(s) for s in {e(LSTR)}}}",
                lambda: f"dict(zip({e(LSTR)}, {e(LINT)}))",
                lambda: f"{{**{e(DICT)}, **{e(DICT)}}}",
                lambda: f"({e(DICT)} | {e(DICT)})",
                lambda: f"dict(sorted({e(DICT)}.items()))",
                lambda: f"{{k: v * 2 for k, v in {e(DICT)}.items() if v > {e(INT)}}}",
            ])()
        if t == TUP:
            return r.choice([
                lambda: f"({e(INT)}, {e(STR)})",
                lambda: f"min([({e(INT)}, {e(STR)}), ({e(INT)}, {e(STR)})])",
                lambda: f"(divmod({e(INT)}, {r.choice(['3', '-7'])})[0], {e(STR)})",
            ])()
        if t == SETI:
            return r.choice([
                lambda: f"({e(SETI)} {r.choice(['|', '&', '-', '^'])} {e(SETI)})",
                lambda: f"set({e(LINT)})",
                lambda: f"{{x % 5 for x in {e(LINT)}}}",
            ])()
        raise ValueError(t)

    # ---- an expression that may raise, for try/except
    def risky(self):
        r = self.r
        e = self.expr
        return r.choice([
            lambda: f"{e(INT)} // {e(INT)}",
            lambda: f"{e(INT)} % {e(INT)}",
            lambda: f"{e(FLOAT)} / {e(INT)}",
            lambda: f"{e(LINT)}[{e(INT)}]",
            lambda: f"{e(STR)}[{e(INT)}]",
            lambda: f"{e(DICT)}[{self.lit(STR)}]",
            lambda: f"int({e(STR)})",
            lambda: f"float({e(STR)})",
            lambda: f"{e(LINT)}.index({e(INT)})",
            lambda: f"{e(STR)}.index({self.lit(STR)})",
            lambda: f"max({e(LINT)})",
            lambda: f"{e(LINT)}.pop()",
            lambda: f"{e(SETI)}.pop() if False else sorted({e(SETI)})[{e(INT)}]",
            lambda: f"{e(DICT)}.pop({self.lit(STR)})",
            lambda: f"[1, 2][{e(INT)}:{e(INT)}][0]",
            lambda: f"{e(STR)} + {e(INT)}",
            lambda: f"{e(INT)} < {e(STR)}",
            lambda: f"divmod({e(INT)}, {e(INT)})",
            lambda: f"{e(INT)} ** -1 if {e(INT)} == 0 else 0 ** -1",
            lambda: f"chr({e(INT)})",
            lambda: f"'abc'.encode('ascii')[{e(INT)}]",
            lambda: f"(lambda *a: a[{r.randint(0, 3)}])({e(INT)}, {e(STR)})",
        ])()

    # ---- statements
    def stmts(self, ind, n):
        out = []
        for _ in range(n):
            out.extend(self.stmt(ind))
        return out

    def stmt(self, ind):
        r = self.r
        p = "    " * ind
        e = self.expr
        self.depth += 1
        try:
            k = r.random()
            deep = self.depth > 3
            if k < 0.22 or not self.vars:
                t = r.choice(TYPES)
                name = self.fresh(t)
                line = f"{p}{name} = {e(t)}"
                self.vars[name] = t
                return [line]
            if k < 0.36:
                return [f"{p}print({', '.join(e(r.choice(TYPES)) for _ in range(r.randint(1, 3)))})"]
            if k < 0.44:
                return [f"{p}try:", f"{p}    print({self.risky()})", f"{p}except (ZeroDivisionError, IndexError, KeyError, ValueError, TypeError, OverflowError) as ex:", f"{p}    print(type(ex).__name__)"]
            if k < 0.52:
                v = self.pick(LINT)
                if v:
                    return [f"{p}{v}.{r.choice(['append', 'remove' if False else 'append'])}({e(INT)})" if r.random() < 0.6 else f"{p}{v}.{r.choice(['sort()', 'reverse()', 'extend(' + e(LINT) + ')', 'insert(' + e(INT) + ', ' + e(INT) + ')', 'clear()'])}"]
                v = self.pick(DICT)
                if v:
                    return [f"{p}{v}[{self.lit(STR)}] = {e(INT)}"]
                return []
            if k < 0.58:
                v = self.pick(INT)
                if v:
                    return [f"{p}{v} {r.choice(['+=', '-=', '*=', '//= 1 or 1 and'])} {e(INT)}".replace("//= 1 or 1 and", "+=")]
                return []
            if k < 0.66 and not deep:
                saved = dict(self.vars)
                cond = e(BOOL)
                body = self.block(ind + 1)
                self.vars = dict(saved)
                orelse = [f"{p}else:"] + self.block(ind + 1) if r.random() < 0.5 else []
                self.vars = saved  # a name set in only one branch may be unbound after the if
                return [f"{p}if {cond}:"] + body + orelse
            if k < 0.74 and not deep:
                src = r.choice([f"range({r.randint(0, 6)})", e(LINT), e(LSTR), e(STR), f"{e(DICT)}.items()", f"enumerate({e(LINT)})"])
                target = "i"
                if ".items()" in src or "enumerate" in src:
                    target = "i, j"
                saved = dict(self.vars)
                lines = [f"{p}for {target} in {src}:"]
                lines += self.block(ind + 1, extra=r.random() < 0.2 and [f"{p}    if {e(BOOL)}:", f"{p}        {r.choice(['break', 'continue'])}"])
                self.vars = saved
                return lines
            if k < 0.78 and not deep:
                c = self.fresh(INT)
                lines = [f"{p}{c} = 0", f"{p}while {c} < {r.randint(0, 5)}:", f"{p}    {c} += 1"]
                saved = dict(self.vars)
                lines += self.block(ind + 1)
                self.vars = saved
                self.vars[c] = INT
                return lines
            if k < 0.86 and not deep:
                f = f"f{self.n + 1}"
                self.n += 1
                pt = [r.choice(TYPES) for _ in range(r.randint(0, 3))]
                rt = r.choice(TYPES)
                saved = dict(self.vars)
                params = []
                for i, t in enumerate(pt):
                    nm = f"p{self.n}_{i}"
                    params.append(nm)
                self.vars = {nm: t for nm, t in zip(params, pt)}
                body = self.block(ind + 1, n=r.randint(1, 3))
                ret = e(rt)
                self.vars = saved
                deflt = ""
                lines = [f"{p}def {f}({', '.join(params)}{deflt}):"] + body + [f"{p}    return {ret}"]
                args = ", ".join(e(t) for t in pt)
                name = self.fresh(rt)
                lines.append(f"{p}{name} = {f}({args})")
                self.vars[name] = rt
                return lines
            if k < 0.92 and not deep:
                cn = f"C{self.n + 1}"
                self.n += 1
                a, b = self.lit(INT), self.lit(STR)
                lines = [
                    f"{p}class {cn}:",
                    f"{p}    def __init__(self, a, b):",
                    f"{p}        self.a = a",
                    f"{p}        self.b = b",
                    f"{p}    def total(self, k):",
                    f"{p}        return self.a * k + len(self.b)",
                    f"{p}    def __repr__(self):",
                    f"{p}        return f'{cn}({{self.a!r}}, {{self.b!r}})'",
                    f"{p}    def __eq__(self, o):",
                    f"{p}        return isinstance(o, {cn}) and self.a == o.a",
                    f"{p}    def __lt__(self, o):",
                    f"{p}        return self.a < o.a",
                    f"{p}    __hash__ = None" if r.random() < 0.3 else f"{p}    def __hash__(self):\n{p}        return hash(self.a)",
                ]
                o = self.fresh(INT)
                lines += [f"{p}{o}s = sorted([{cn}({e(INT)}, {e(STR)}) for _ in range({r.randint(0, 4)})] + [{cn}({a}, {b})])",
                          f"{p}print({o}s, {o}s[0].total({e(INT)}), {o}s[-1] == {cn}({a}, 'q'), max({o}s))"]
                return lines
            if k < 0.96:
                v = self.pick(LINT)
                t = self.pick(TUP)
                if t and r.random() < 0.5:
                    a, b = self.fresh(INT), self.fresh(STR)
                    self.vars[a], self.vars[b] = INT, STR
                    return [f"{p}{a}, {b} = {t}"]
                if v:
                    return [f"{p}{v}[{r.choice(['0:1', '1:', ':0', '::2'])}] = {e(LINT)}" if r.random() < 0.5 else f"{p}del {v}[{r.choice(['0:1', '::2', ':'])}]"]
                return []
            return [f"{p}print(sorted({e(DICT)}.items()), {e(SETI)} == {e(SETI)})"]
        finally:
            self.depth -= 1

    def block(self, ind, n=None, extra=None):
        body = self.stmts(ind, n if n is not None else self.r.randint(1, 3))
        if extra:
            body = extra + body
        return body or ["    " * ind + "pass"]

# ---- aliasing cases: two names for one list/dict (Python shares the object; a value-copy build must not copy)

def _ints(r, lo=3, hi=6):
    return "[" + ", ".join(str(r.randint(-9, 30)) for _ in range(r.randint(lo, hi))) + "]"


def _mutations(r, m, helper):
    """Statements (no indent) that mutate the int list named m in place; never raise, never empty it unless clearing."""
    k = r.randint(0, 11)
    if k == 0: return [f"{m}.append({r.randint(-5, 99)})"]
    if k == 1: return [f"{m}.extend([{r.randint(0, 9)}, {r.randint(0, 9)}])"]
    if k == 2: return [f"{m}.sort()"]
    if k == 3: return [f"{m}.reverse()"]
    if k == 4: return [f"{m}.pop()"]
    if k == 5: return [f"{m}.clear()"]
    if k == 6: return [f"{m}[0] = {r.randint(100, 199)}"]
    if k == 7: return [f"del {m}[0]"]
    if k == 8: return [f"{m} += [{r.randint(0, 9)}]"]
    if k == 9: return [f"{helper}({m})"]
    if k == 10: return [f"{m}.insert(1, {r.randint(0, 9)})"]
    return [f"{m}[-1] += {r.randint(1, 9)}", f"{m}.sort(reverse=True)"]


def _show(r, names):
    """A print of every name, in a varied way (the list itself, len, sum, a slice)."""
    out = []
    for n in names:
        k = r.randint(0, 3)
        out.append([f"print({n})", f"print(len({n}), sum({n}))", f"print({n}, len({n}))", f"print({n}[:2], sum({n}))"][k])
    return out


def gen_alias_case(rng, i):
    """One aliasing case body (list of lines, indent 1): make a second reference to a list, mutate through one name,
    print both. Some creators copy on purpose (slice, list(), .copy()): those are the non-sharing controls."""
    r = rng
    PRE = []  # module-level lines before the def (a class nested in a function sends the whole program down the dynamic path)
    L = ["    a = " + _ints(r)]
    helper = r.choice(["bump", "mutate", "shuffle_in_place"])
    hk = r.randint(0, 3)
    L += [f"    def {helper}(xs):",
          [f"        xs.append({r.randint(1, 9)})", "        xs.sort()", "        xs.reverse()", "        xs[0] = -1"][hk]]
    sc = r.choice(["generic"] * 6 + ["smallest", "rows", "store", "nested", "cls", "iter", "dictlist", "tuple", "retparam", "dict"])
    if sc == "generic":
        kind = r.choice(["assign", "pass", "append", "dictval", "tuple", "field", "loop", "retparam", "retfield",
                         "slice", "list", "copy", "plus"])
        if kind == "assign": L += ["    b = a"]
        elif kind == "pass":
            L += ["    def keep(xs, store):", "        store.append(xs)", "    store = []", "    keep(a, store)", "    b = store[0]"]
        elif kind == "append": L += ["    holder = [[9]]", "    holder.append(a)", "    b = holder[-1]"]
        elif kind == "dictval": L += ["    d = {'k': a}", "    b = d['k']"]
        elif kind == "tuple": L += ["    t = (a, 1)", "    b = t[0]"]
        elif kind in ("field", "retfield"):
            L += [f"    bx = Box{i}(a)"]
            PRE += [f"class Box{i}:", "    def __init__(self, items):", "        self.items = items",
                    "    def get(self):", "        return self.items", ""]
            L += ["    b = bx.items" if kind == "field" else "    b = bx.get()"]
        elif kind == "loop": L += ["    for b in [a]:", "        pass"]
        elif kind == "retparam": L += ["    def ident(xs):", "        return xs", "    b = ident(a)"]
        elif kind == "slice": L += ["    b = a[:]"]
        elif kind == "list": L += ["    b = list(a)"]
        elif kind == "copy": L += ["    b = a.copy()"]
        else: L += ["    b = a + []"]
        for _ in range(r.randint(1, 2)):
            L += ["    " + s for s in _mutations(r, r.choice(["a", "b"]), helper)]
        L += ["    " + s for s in _show(r, ["a", "b"])]
        if r.random() < 0.3:
            L += [f"    print(a is b, a == b)"]
    elif sc == "smallest":
        L += ["    def smallest(xs):", "        xs.sort()", "        return xs[0]",
              "    print(smallest(a))", "    print(a)"]
    elif sc == "rows":
        L += ["    rows = []", "    row = [0, 0]", f"    for k in range({r.randint(2, 4)}):", "        row[0] = k"]
        if r.random() < 0.5:
            L += ["        rows.append(row)"]
        else:
            L += ["        rows.append(row[:])"]
        L += ["    print(rows)", "    print(len(rows), sum(x[0] for x in rows))"]
    elif sc == "store":
        L += ["    def keep(xs, store):", "        store.append(xs)", "    store = []", "    keep(a, store)"]
        L += ["    " + s for s in _mutations(r, "a", helper)]
        L += ["    print(store)", "    print(a)", "    print(len(store[0]))"]
    elif sc == "nested":
        L += ["    a = [[1, 2], [3]]", "    b = [a[0], a[1]]" if r.random() < 0.7 else "    b = [x[:] for x in a]",
              f"    b[0].append({r.randint(5, 9)})", "    b[1].extend([7, 8])", "    print(a)", "    print(b)"]
    elif sc == "cls":
        PRE += [f"class Bag{i}:", "    def __init__(self, items):", "        self.items = items",
                "    def order(self):", "        self.items.sort()", "        return len(self.items)",
                "    def add(self, x):", "        self.items.append(x)", ""]
        L += [f"    bag = Bag{i}(a)", "    print(bag.order())", "    bag.add(0)" if r.random() < 0.5 else "    pass",
              "    print(a)", "    print(bag.items)"]
    elif sc == "iter":
        k = r.randint(0, 2)
        if k == 0:
            L += ["    for n in a:", "        a.clear()", "    print(a)"]
        elif k == 1:
            L += ["    out = []", "    for n in a:", "        out.append(n)", "        if n >= 0:", "            a.append(0)", "            break",
                  "    print(out, a)"]
        else:
            L += ["    for i, x in enumerate(a):", "        if x >= 0:", "            del a[i]", "            break", "    print(a, len(a))"]
    elif sc == "dictlist":
        L += ["    d = {'x': a, 'y': [5, 1, 3]}", "    v = d['x']", f"    v.append({r.randint(0, 9)})",
              "    for k, w in d.items():", "        w.sort()", "    print(sorted(d.items()))", "    print(a)"]
    elif sc == "tuple":
        L += ["    t = (a, len(a))"]
        L += ["    " + s for s in _mutations(r, "t[0]", helper)]
        L += ["    print(t)", "    print(a)"]
    elif sc == "retparam":
        L += ["    def first_or(xs):", "        return xs", "    c = first_or(a)", "    d = first_or(c)"]
        L += ["    " + s for s in _mutations(r, r.choice(["c", "d"]), helper)]
        L += ["    print(a, c, d)", "    print(len(a), sum(d))"]
    else:  # dict aliasing
        L = ["    a = {1: 'one', 2: 'two'}", "    b = a" if r.random() < 0.7 else "    b = dict(a)"]
        m = r.choice(["a", "b"])
        L += r.choice([[f"    {m}[3] = 'three'"], [f"    {m}.pop(1)"], [f"    {m}.update({{9: 'nine'}})"], [f"    {m}.clear()"],
                       [f"    del {m}[2]"], [f"    {m}.setdefault(7, 'seven')"]])
        L += ["    print(sorted(a.items()))", "    print(sorted(b.items()), len(a), len(b))"]
    return PRE, L


ALIAS_ONLY = False


def case_source(seed, batch, i):
    """One case: a function whose body is generated from (seed, batch, i) alone, so a case reproduces by itself."""
    g = Gen(random.Random(f"{seed}:{batch}:{i}"))
    if ALIAS_ONLY or g.r.random() < 0.25:
        pre, body = gen_alias_case(g.r, i)
        return "\n".join(pre + [f"def case_{i}():"] + body)
    body = g.stmts(1, g.r.randint(3, 9))
    if not any(l.lstrip().startswith("print") for l in body):
        body.append(f"    print({', '.join(sorted(g.vars)) or '0'})")
    return "\n".join([f"def case_{i}():"] + body)


def program(cases):
    """cases: [(i, source)]. Each is called under its own try/except, so one raising does not hide the rest."""
    out = ["# generated by tools/fuzz.py", ""]
    for _, src in cases:
        out += [src, ""]
    # type(ex) is "unknown function type()" to Brevi and sends the whole program down the dynamic path, where lists are
    # shared objects. --alias-only prints a bare 'raised' so the typed build (the one that copies) is what gets tested.
    raised = "print('raised')" if ALIAS_ONLY else "print('raised', type(ex).__name__)"
    for i, _ in cases:
        out += [f"print('-- case {i}')", "try:", f"    case_{i}()", "except Exception as ex:", f"    {raised}"]
    return "\n".join(out) + "\n"


# ---------------------------------------------------------------- running

def run(cmd, cwd, env=None, timeout=TIMEOUT):
    try:
        r = subprocess.run(cmd, cwd=cwd, capture_output=True, timeout=timeout, env=env)
        return r.returncode, r.stdout, r.stderr
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""


def judge(src, work):
    """Run one program under CPython, then both builds. Returns (verdict, detail) per mode, or None if CPython
    rejects it. verdict is ok, refused, panic, signal, wrong, build or timeout."""
    work.mkdir(parents=True, exist_ok=True)
    f = work / "case.py"
    f.write_text(src)
    env0 = dict(os.environ, PYTHONHASHSEED="0")
    py = run([PYTHON, str(f)], work, env0)
    if py[0] == "timeout" or py[0] != 0:
        return None
    res = {}
    for mode in ("normal", "dyn"):
        env = dict(os.environ, BREVI_TIERED="0")
        if mode == "dyn":
            env["BREVI_DYN"] = "1"
        exe = work / f"prog_{mode}"
        b = run([BREVI, "build", str(f), "-o", str(exe)], work, env, timeout=300)
        if b[0] == "timeout":
            res[mode] = ("build", "build timed out")
            continue
        if b[0] != 0:
            err = (b[2] + b[1]).decode(errors="replace")  # a build error is a JSON line on stdout
            if "panicked at" in err or (isinstance(b[0], int) and b[0] < 0):
                res[mode] = ("build", first_panic(err) or f"signal {-b[0]}")
            else:
                res[mode] = ("refused", err.strip().splitlines()[-1][:200] if err.strip() else "")
            continue
        g = run([str(exe)], work, env0)
        if g[0] == "timeout":
            res[mode] = ("timeout", "")
        elif b"panicked at" in g[2]:
            res[mode] = ("panic", first_panic(g[2].decode(errors="replace")))
        elif g[0] < 0:
            res[mode] = ("signal", f"signal {-g[0]}")
        elif g[0] != py[0] or g[1] != py[1]:
            res[mode] = ("wrong", first_diff(py[1], g[1]) if g[1] != py[1] else f"exit {g[0]}, expected {py[0]}: {g[2].decode(errors='replace').strip()[-200:]}")
        else:
            res[mode] = ("ok", "")
    return res


def first_panic(err):
    for ln in err.splitlines():
        if "panicked at" in ln:
            i = err.splitlines().index(ln)
            nxt = err.splitlines()[i + 1] if i + 1 < len(err.splitlines()) else ""
            return (re.sub(r"/[^ ]*/(rt\.rs|[a-z_]+\.rs)", r"\1", ln.strip()) + " " + nxt.strip())[:240]
    return ""


def first_diff(a, b):
    al, bl = a.decode(errors="replace").splitlines(), b.decode(errors="replace").splitlines()
    for i in range(max(len(al), len(bl))):
        x = al[i] if i < len(al) else "<end>"
        y = bl[i] if i < len(bl) else "<end>"
        if x != y:
            return f"line {i + 1}: expected {x[:120]!r}, got {y[:120]!r}"
    return ""


BAD = ["panic", "signal", "build", "timeout", "wrong"]


def reduce(src, still_bad, tmp):
    """Drop statements (deepest bodies first, then whole cases) while still_bad(source) holds; return the smallest."""
    import ast

    def bodies(tree):
        out = []
        for node in ast.walk(tree):
            for field in ("body", "orelse", "handlers", "finalbody"):
                b = getattr(node, field, None)
                if isinstance(b, list) and b and isinstance(b[0], (ast.stmt, ast.excepthandler)):
                    out.append((node, field))
        return out

    best = src
    changed = True
    while changed:
        changed = False
        tree = ast.parse(best)
        for node, field in reversed(bodies(tree)):
            b = getattr(node, field)
            k = len(b) - 1
            while k >= 0 and len(b) > 0:
                keep = b[k]
                del b[k]
                if not b:
                    b.append(ast.Pass())
                try:
                    cand = ast.unparse(tree) + "\n"
                except Exception:
                    cand = None
                if cand and cand != best and still_bad(cand):
                    best, changed = cand, True
                    if b and isinstance(b[0], ast.Pass) and len(b) == 1:
                        pass
                else:
                    if len(b) == 1 and isinstance(b[0], ast.Pass):
                        b.clear()
                    b.insert(k, keep)
                k -= 1
    return best


def worst(res):
    vs = [v for v, _ in res.values()]
    for b in BAD:
        if b in vs:
            return b
    return "ok"


def do_batch(seed, batch, ncases, tmp):
    cases = [(i, case_source(seed, batch, i)) for i in range(ncases)]
    work = Path(tmp) / f"b{seed}_{batch}"
    res = judge(program(cases), work)
    if res is None:
        # CPython rejected the batch (a generator bug); judge the cases one at a time instead.
        found = []
        for i, src in cases:
            r1 = judge(program([(i, src)]), work / f"c{i}")
            if r1 and worst(r1) != "ok":
                found.append((i, src, r1))
        shutil.rmtree(work, ignore_errors=True)
        return {"batch": batch, "skipped": True, "found": found, "res": None}
    found = []
    if worst(res) != "ok":
        for i, src in cases:
            r1 = judge(program([(i, src)]), work / f"c{i}")
            if r1 and worst(r1) != "ok":
                found.append((i, src, r1))
        if not found:
            found.append((None, program(cases), res))  # only fails together
    shutil.rmtree(work, ignore_errors=True)
    return {"batch": batch, "skipped": False, "found": found, "res": res}


def save(out, seed, batch, i, src, res):
    out.mkdir(parents=True, exist_ok=True)
    kind = worst(res)
    name = f"{kind}_s{seed}_b{batch}_c{i if i is not None else 'all'}.py"
    note = "; ".join(f"{m}: {v} {d}" for m, (v, d) in res.items() if v != "ok")
    text = src if i is None else program([(i, src)])
    (out / name).write_text(f"# tools/fuzz.py seed {seed} batch {batch} case {i}: {note}\n" + text)
    return out / name, kind, note


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--batches", type=int, default=20)
    ap.add_argument("--cases", type=int, default=12, help="cases per batch (one build per batch per mode)")
    ap.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 2) // 2))
    ap.add_argument("--out", default=str(ROOT / "bench" / "fuzz" / "found"))
    ap.add_argument("--fail-on", default="panic,signal,build")
    ap.add_argument("--show", help="SEED:BATCH, print that batch's program")
    ap.add_argument("--replay", help="a folder of saved cases: each must match CPython on both builds")
    ap.add_argument("--alias-only", action="store_true", help="generate only aliasing cases (two names, one list/dict)")
    ap.add_argument("--json", help="write a summary here")
    ap.add_argument("--reduce", help="a saved failing case: print the smallest program that still fails the same way")
    a = ap.parse_args()
    fail_on = set(a.fail_on.split(","))
    global ALIAS_ONLY
    ALIAS_ONLY = a.alias_only
    if a.show:
        s, b = a.show.split(":")
        print(program([(i, case_source(int(s), int(b), i)) for i in range(a.cases)]), end="")
        return 0
    if not Path(BREVI).exists():
        sys.exit(f"no {BREVI}: cargo build --release first")
    if a.reduce:
        tmpd = Path(tempfile.mkdtemp(prefix="brevi-fuzz-"))
        src = Path(a.reduce).read_text()
        first = judge(src, tmpd / "orig")
        if not first or worst(first) == "ok":
            sys.exit("that case does not fail now")
        want = {m: v for m, (v, _) in first.items() if v != "ok"}
        step = [0]

        def still_bad(cand):
            step[0] += 1
            r = judge(cand, tmpd / f"r{step[0]}")
            return bool(r) and all(r[m][0] == v for m, v in want.items())

        print(reduce(src, still_bad, tmpd), end="")
        shutil.rmtree(tmpd, ignore_errors=True)
        return 0
    t0 = time.time()
    tmp = tempfile.mkdtemp(prefix="brevi-fuzz-")
    counts = {k: 0 for k in BAD + ["ok", "refused"]}
    failed = []
    if a.replay:
        files = sorted(Path(a.replay).glob("*.py"))
        with cf.ThreadPoolExecutor(a.jobs) as ex:
            futs = {ex.submit(judge, f.read_text(), Path(tmp) / f.stem): f for f in files}
            for fu in cf.as_completed(futs):
                f, res = futs[fu], fu.result()
                if res is None:
                    print(f"SKIP {f.name}: CPython no longer runs it")
                    continue
                for v, _ in res.values():
                    counts[v] += 1
                w = worst(res)
                if w != "ok":
                    failed.append(w)
                    print(f"FAIL {f.name}: " + "; ".join(f"{m}: {v} {d}" for m, (v, d) in res.items() if v != "ok"))
        print(f"fuzz replay: {len(files)} cases, {counts}, {time.time() - t0:.0f} s")
        shutil.rmtree(tmp, ignore_errors=True)
        return 1 if failed else 0
    out = Path(a.out)
    skipped = 0
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = [ex.submit(do_batch, a.seed, b, a.cases, tmp) for b in range(a.batches)]
        for n, fu in enumerate(cf.as_completed(futs), 1):
            r = fu.result()
            skipped += r["skipped"]
            if r["res"]:
                for v, _ in r["res"].values():
                    counts[v] += 1
            for i, src, res in r["found"]:
                path, kind, note = save(out, a.seed, r["batch"], i, src, res)
                failed.append(kind)
                print(f"{kind.upper():7} {path.relative_to(ROOT) if path.is_relative_to(ROOT) else path}: {note}", flush=True)
            if n % 10 == 0:
                print(f"  {n}/{a.batches} batches, {time.time() - t0:.0f} s", flush=True)
    shutil.rmtree(tmp, ignore_errors=True)
    kinds = {k: failed.count(k) for k in BAD if failed.count(k)}
    print(f"fuzz: seed {a.seed}, {a.batches} batches x {a.cases} cases ({skipped} batches CPython rejected), "
          f"build results {counts}, failing cases {kinds or 'none'}, {time.time() - t0:.0f} s")
    if a.json:
        Path(a.json).write_text(json.dumps({"seed": a.seed, "batches": a.batches, "cases": a.cases, "counts": counts, "failing": kinds}))
    return 1 if fail_on & set(failed) else 0


if __name__ == "__main__":
    sys.exit(main())
