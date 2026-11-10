#!/usr/bin/env python3
"""How much of Python does Brevi really handle? Measured with CPython's own test suite.

Each test method of a pinned CPython Lib/test file is cut out into a standalone script, with assertions
rewritten to print values. Every script runs under one known CPython, then goes through `brevi build`
twice (the normal path, and BREVI_DYN=1 for the forced dynamic path). The built programs must print the
same stdout and exit with the same code. `brevi run` is never used: it falls back to CPython.

  python3 tools/conformance.py                     measure, write report.json and report.md
  python3 tools/conformance.py --check             the ratchet: exit 1 on a wrong result or crash not in the baseline, on a
                                                   baseline line that is no longer wrong (remove it), or on fewer matches
                                                   than compat/conformance_floor.json allows (CI runs this; see below)
  python3 tools/conformance.py --write-baseline    make the baseline the wrong results and crashes of this run, and the
                                                   floor this run's match counts
  python3 tools/conformance.py --show ID           print one test's generated script and how to reproduce it

Categories: matched, refused, wrong_output, wrong_exit, crash_or_timeout, not_extractable, unstable, and
excluded (CPython internals, each with a reason). coverage = matched / (all - unstable - excluded);
wrong = wrong_output + wrong_exit among programs that built.

The test files are fetched once from the pinned tag into a cache outside git (not vendored: nothing of
CPython's lands in this repository, so no PSF notice has to travel with it, and a new pin is one edit).
Every file is checked against its SHA-256, so later runs need no network. Fetched files are only parsed
with `ast`, never imported or run; only generated scripts run, under a timeout and a memory limit.
Env: BREVI (default target/release/brevi), BREVI_CACHE (the cache is its conformance/ folder, else
~/.cache/brevi/conformance).
"""
import argparse, ast, collections, concurrent.futures as cf, hashlib, json, os, re, shlex, shutil, signal
import subprocess, sys, tempfile, time, urllib.request, warnings
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASELINE = ROOT / "compat" / "conformance_known_wrong.txt"
FLOOR = ROOT / "compat" / "conformance_floor.json"  # the match counts a run may not fall below
BAD = ("wrong_output", "wrong_exit", "crash_or_timeout")  # built, but did not do what CPython does
TAG = "v3.13.13"
PYTHON_VERSION = "3.13.13"
RAW = "https://raw.githubusercontent.com/python/cpython/" + TAG + "/Lib/"
SCOPE = ["test_grammar", "test_scope", "test_int", "test_list", "test_dict"]
# Module name -> (path under Lib/, SHA-256). The helpers are only read for the definitions tests use.
SOURCES = {
    "test.support": ("test/support/__init__.py", "79347b3f68109a2a1315bba9c7769acc631908ade382ddeef473bfb72aa75ca9"),
    "test.support.numbers": ("test/support/numbers.py", "fd9c8f35ef65c32612599a89a3ff8fe320268bd139a1c1a773cdfdd44096202c"),
    "test.seq_tests": ("test/seq_tests.py", "030efe9c5f9b5a7cbe06ab7a5d590e9ec7a6b2b54a22b92183f95b8503f6c19c"),
    "test.list_tests": ("test/list_tests.py", "229ac95acf5e22e24795a69a464da4bde446176df297d1782f4c87e535d80f2f"),
    "test.mapping_tests": ("test/mapping_tests.py", "a9553128a09fc2190fbf085ce19eae3edf7f1b358765dd0ff7880037a9a8460a"),
    "test.test_grammar": ("test/test_grammar.py", "301bccc2db95dc3147d0ef4225a02bfe4eeb73fab62138863594b9851debd993"),
    "test.test_scope": ("test/test_scope.py", "ed697656916d59ca03268751a09f3f552a4bda72ec7ff6c0933602b49c994d62"),
    "test.test_int": ("test/test_int.py", "2d947888f306e945ebc12c6817691330528c0bc7a13330b942c9af97fe3075d8"),
    "test.test_list": ("test/test_list.py", "088eb797e98cbbf6374bd1e646e4d8c6cecfc4ca114873653336e661e7938887"),
    "test.test_dict": ("test/test_dict.py", "009c957818d2e4c13b5c1e5a2bf90c7122ad77ac17362f8ef3ec6b8f6bf61f06"),
}

# A generated script may import only these: nothing that reaches files, processes or the network.
ALLOWED_IMPORTS = {
    "__future__", "abc", "array", "bisect", "builtins", "cmath", "collections", "contextlib", "copy", "dataclasses",
    "decimal", "enum", "fractions", "functools", "gc", "heapq", "inspect", "itertools", "json", "math", "numbers",
    "operator", "pickle", "random", "re", "string", "struct", "sys", "textwrap", "time", "types", "typing",
    "unicodedata", "warnings", "weakref",
}
FORBIDDEN_NAMES = {"open", "__import__", "breakpoint", "input"}
# Tests of CPython itself, not of the language: left out of the total, each with its reason.
EXCLUDED_DECORATORS = {
    "cpython_only": "marked cpython_only",
    "suppress_immortalization": "tests CPython's immortal objects",
    "requires_resource": "needs an opt-in test resource",
    "bigmemtest": "needs gigabytes of memory",
}
EXCLUDED_CLASSES = {
    "PyLongModuleTests": "tests _pylong, a private CPython module",
    "CAPITest": "tests the C API",
}
EXCLUDED_SOURCE = [
    (r"\b_test(capi|internalcapi|limitedcapi)\b", "tests the C API"),
    (r"\bsys\.getrefcount\b|\bcheck_free_after_iterating\b", "depends on reference counts"),
    (r"\bsys\.getsizeof\b|\b__sizeof__\b", "depends on object sizes"),
    (r"\bgc\.is_tracked\b|\bcheck_impl_detail\b|\brun_in_subinterp\b", "tests CPython internals"),
    (r"\bCPUStopwatch\b", "measures CPU time"),
]
RECEIVERS = {"self", "testcase"}
SAME_AS_EQUAL = {"assertEqual", "assertListEqual", "assertTupleEqual", "assertDictEqual", "assertSetEqual",
                 "assertSequenceEqual", "assertMultiLineEqual"}
COMPARE = {"assertNotEqual": "!=", "assertIs": "is", "assertIsNot": "is not", "assertIn": "in", "assertNotIn": "not in",
           "assertLess": "<", "assertLessEqual": "<=", "assertGreater": ">", "assertGreaterEqual": ">="}
TEST_API = {"fail", "skipTest", "subTest", "addCleanup", "addTypeEqualityFunc", "doCleanups", "enterContext"}
NO_RAISE = "NO-RAISE"  # printed where an assertRaises saw nothing: the extraction changed the test


class CantExtract(Exception):
    pass


class Excluded(Exception):
    pass


def parse(src):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return ast.parse(src)


class Mod:
    """One fetched source file: its top-level definitions, and the edits that rewrite its assertions.

    Edits are made on the source text, not the tree, because ast.unparse would turn 0xff into 255 and
    join implicit string concatenations, which is what test_grammar is testing."""

    def __init__(self, name, data):
        self.name, self.data, self.tree = name, data, parse(data)
        self.starts = [0]
        for line in data.split(b"\n"):
            self.starts.append(self.starts[-1] + len(line) + 1)
        self.edits, self.defs, self.imports, self.stars = [], {}, {}, []
        for node in self.tree.body:
            self.index(node)

    def index(self, node):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            self.defs.setdefault(node.name, []).append(node)
        elif isinstance(node, ast.Import):
            for a in node.names:
                self.imports[a.asname or a.name.split(".")[0]] = ("import", a.name, a.asname)
        elif isinstance(node, ast.ImportFrom):
            for a in node.names:
                if a.name == "*":
                    self.stars.append(node.module)
                else:
                    self.imports[a.asname or a.name] = ("from", node.module, a.name)
        elif not (isinstance(node, ast.If) and "__name__" in ast.dump(node.test)):
            for sub in ast.walk(node):
                if isinstance(sub, ast.Name) and isinstance(sub.ctx, ast.Store):
                    self.defs.setdefault(sub.id, []).append(node)

    def off(self, line, col):
        return self.starts[line - 1] + col

    def span(self, node):
        first = min([node.lineno] + [d.lineno for d in getattr(node, "decorator_list", [])])
        col = node.col_offset if first == node.lineno else 0
        return self.off(first, col), self.off(node.end_lineno, node.end_col_offset)

    def edit(self, start, end, text):
        self.edits = [e for e in self.edits if not (start <= e[0] and e[1] <= end and e[0] != e[1])]
        self.edits.append((start, end, text.encode()))

    def text(self, start, end):
        out, pos = [], start
        for s, e, t in sorted((e for e in self.edits if start <= e[0] and e[1] <= end), key=lambda e: (e[0], e[1])):
            out += [self.data[pos:s], t]
            pos = e
        out.append(self.data[pos:end])
        return b"".join(out).decode()

    def expr(self, node):
        return self.text(*self.span(node))

    def block(self, node):
        """The rewritten source of a statement, from the start of its first line."""
        start, end = self.span(node)
        return self.text(self.starts[min([node.lineno] + [d.lineno for d in getattr(node, "decorator_list", [])]) - 1], end)

    def alone(self, node):
        """Whether a statement has its lines to itself, so it can become several lines."""
        start = self.starts[node.lineno - 1]
        before = self.data[start:self.off(node.lineno, node.col_offset)]
        after = self.data[self.off(node.end_lineno, node.end_col_offset):self.starts[node.end_lineno] - 1]
        return not before.strip() and re.fullmatch(rb"\s*(#.*)?", after) is not None


class Rewriter(ast.NodeVisitor):
    """Replaces assertion statements with prints. What it cannot rewrite stays in the text, and the
    final check on the generated script then records the test as not_extractable."""

    def __init__(self, mod, helper_modules):
        self.m, self.helpers, self.caught = mod, helper_modules, set()

    def visit_FunctionDef(self, node):
        saved, self.caught = self.caught, set()
        self.generic_visit(node)
        self.caught = saved

    def visit_Attribute(self, node):
        self.generic_visit(node)
        v = node.value
        if not isinstance(v, ast.Name):
            return
        if node.attr == "exception" and v.id in self.caught:
            self.m.edit(*self.m.span(node), v.id + "_exception")
        kind = self.m.imports.get(v.id)
        if kind and kind[0] == "from" and f"{kind[1]}.{kind[2]}" in self.helpers:
            self.m.imports.setdefault(node.attr, ("from", f"{kind[1]}.{kind[2]}", node.attr))
            self.m.edit(*self.m.span(node), node.attr)

    def visit_Expr(self, node):
        self.generic_visit(node)
        call = node.value
        if not isinstance(call, ast.Call):
            return
        f = call.func
        if isinstance(f, ast.Attribute) and f.attr == "__init__" and "TestCase" in ast.dump(f.value):
            self.m.edit(*self.m.span(node), "pass")
        elif isinstance(f, ast.Attribute) and isinstance(f.value, ast.Name) and f.value.id in RECEIVERS:
            lines = rewrite_assertion(self.m, node, f.attr, call)
            if lines:
                sep = "\n" + " " * node.col_offset if self.m.alone(node) else "; "
                if "\n" not in sep and any(ln.startswith((" ", "try:", "except", "else:")) for ln in lines):
                    return
                self.m.edit(*self.m.span(node), sep.join(lines))

    def visit_With(self, node):
        self.generic_visit(node)
        if len(node.items) != 1:
            return
        item = node.items[0]
        c = item.context_expr
        if not (isinstance(c, ast.Call) and isinstance(c.func, ast.Attribute) and isinstance(c.func.value, ast.Name)
                and c.func.value.id in RECEIVERS):
            return
        last = item.optional_vars or c
        header_end = self.m.data.index(b":", self.m.off(last.end_lineno, last.end_col_offset)) + 1
        start = self.m.off(node.lineno, node.col_offset)
        ind = " " * node.col_offset
        if c.func.attr == "subTest":
            self.m.edit(start, header_end, "if True:")
        elif c.func.attr in ("assertRaises", "assertRaisesRegex") and c.args:
            lines = caught_lines(self.m.expr(c.args[0]), c.func.attr == "assertRaisesRegex")
            if item.optional_vars is not None:
                if not isinstance(item.optional_vars, ast.Name):
                    return
                name = item.optional_vars.id
                self.caught.add(name)
                lines.insert(2, f"    {name}_exception = _e")
                self.fix_uses(node, name)
            self.m.edit(start, header_end, "try:")
            end = self.m.off(node.end_lineno, node.end_col_offset)
            self.m.edit(end, end, "".join("\n" + ind + ln for ln in lines))

    def fix_uses(self, node, name):
        """`cm.exception` read inside the with body itself, before the name was known."""
        for sub in ast.walk(node):
            if isinstance(sub, ast.Attribute) and sub.attr == "exception" and isinstance(sub.value, ast.Name) and sub.value.id == name:
                self.m.edit(*self.m.span(sub), name + "_exception")


def caught_lines(exc, with_message):
    shown = "type(_e).__name__, str(_e)" if with_message else "type(_e).__name__"
    return [f"except {exc} as _e:", f"    print('raises', {shown})", "else:", f"    print('{NO_RAISE}')"]


def rewrite_assertion(m, stmt, name, call):
    """The lines that replace `self.<name>(...)`, or None to leave it for the final check."""
    if any(k.arg not in ("msg", None) for k in call.keywords) and not name.startswith("assertRaises"):
        return None
    args = call.args
    if any(isinstance(a, ast.Starred) for a in args[:2]):
        return None
    t = [m.expr(a) for a in args]
    if name in SAME_AS_EQUAL and len(t) >= 2:
        b = args[1]
        plain = isinstance(b, ast.Constant) and t[1] == repr(b.value)
        return [f"print(repr({t[0]}))"] + ([] if plain else [f"print(repr({t[1]}))"])
    if name in COMPARE and len(t) >= 2:
        return [f"print(({t[0]}) {COMPARE[name]} ({t[1]}))"]
    if name == "assertTrue" or name == "assertFalse":
        return [f"print(repr(bool({t[0]})))"] if t else None
    if name == "assertIsNone":
        return [f"print(repr({t[0]}))"] if t else None
    if name == "assertIsNotNone":
        return [f"print(({t[0]}) is not None)"] if t else None
    if name == "assertIsInstance" and len(t) >= 2:
        return [f"print(isinstance({t[0]}, {t[1]}))"]
    if name == "assertNotIsInstance" and len(t) >= 2:
        return [f"print(not isinstance({t[0]}, {t[1]}))"]
    if name == "assertAlmostEqual" and len(t) == 2 and not call.keywords:
        return [f"print(round(({t[0]}) - ({t[1]}), 7) == 0)"]
    if name == "fail":
        return [f"raise AssertionError({t[0] if t else ''})"]
    if name in ("assertRaises", "assertRaisesRegex"):
        skip = 2 if name == "assertRaisesRegex" else 1
        if len(args) <= skip or not m.alone(stmt):
            return None
        fn = args[skip]
        callee = t[skip] if isinstance(fn, (ast.Name, ast.Attribute)) else f"({t[skip]})"
        rest = t[skip + 1:] + [m.expr(k) for k in call.keywords]
        return ["try:", f"    {callee}({', '.join(rest)})"] + caught_lines(t[0], name == "assertRaisesRegex")
    return None


def dedent(text, n):
    """Takes n spaces off each line, except lines inside a multi-line string: their spaces are data."""
    keep = set()
    for node in ast.walk(parse("class _:\n" + text)):
        if isinstance(node, (ast.Constant, ast.JoinedStr)) and getattr(node, "end_lineno", None) and node.end_lineno > node.lineno:
            keep.update(range(node.lineno - 1, node.end_lineno - 1))  # indexes in text: the wrapper is line 1
    lines = text.split("\n")
    return "\n".join(ln if i in keep else (ln[n:] if ln[:n].isspace() else ln.lstrip()) for i, ln in enumerate(lines))


def uses(text):
    """Names a piece of code reads from outside, and the attributes it reads off self or cls."""
    tree = parse("class _:\n" + text if text[:1].isspace() else text)
    loaded, stored, attrs = set(), set(), set()
    for n in ast.walk(tree):
        if isinstance(n, ast.Name):
            (loaded if isinstance(n.ctx, ast.Load) else stored).add(n.id)
        elif isinstance(n, ast.arg):
            stored.add(n.arg)
        elif isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) and n.name != "_":
            stored.add(n.name)
        elif isinstance(n, ast.alias):
            stored.add(n.asname or n.name.split(".")[0])
        if isinstance(n, ast.Attribute) and isinstance(n.value, ast.Name) and n.value.id in ("self", "cls"):
            attrs.add(n.attr)
    return loaded - stored, attrs


class Universe:
    """All fetched modules, and how to cut one test out of them."""

    def __init__(self, sources):
        names = set(sources)
        self.mods = {}
        for name, data in sources.items():
            m = Mod(name, data)
            self.mods[name] = m
            Rewriter(m, names).visit(m.tree)
        self.order = list(sources)

    def chain(self, mod, cls):
        """The class and its bases, nearest first, ending before unittest.TestCase; None if not a test."""
        out = [(mod, cls)]
        bases = [b for b in cls.bases if not (isinstance(b, ast.Name) and b.id == "object")]
        if not bases:
            return None
        if len(bases) > 1:
            raise CantExtract("more than one base class")
        b = bases[0]
        if isinstance(b, ast.Attribute) and isinstance(b.value, ast.Name) and b.attr == "TestCase" and b.value.id == "unittest":
            return out
        target = None
        if isinstance(b, ast.Name):
            target = (mod, b.id)
        elif isinstance(b, ast.Attribute) and isinstance(b.value, ast.Name):
            kind = mod.imports.get(b.value.id)
            if kind and kind[0] == "from" and f"{kind[1]}.{kind[2]}" in self.mods:
                target = (self.mods[f"{kind[1]}.{kind[2]}"], b.attr)
        if target is None:
            return None
        found = [n for n in target[0].defs.get(target[1], []) if isinstance(n, ast.ClassDef)]
        rest = self.chain(target[0], found[0]) if found else None
        return out + rest if rest else None

    def tests(self, name):
        """(class, method) for every test the unittest loader would run in this file, inherited ones included."""
        mod = self.mods["test." + name]
        for cls in mod.tree.body:
            if not isinstance(cls, ast.ClassDef):
                continue
            try:
                chain = self.chain(mod, cls)
            except CantExtract:
                chain = [(mod, cls)]
            if not chain:
                continue
            seen = {}
            for _, c in chain:
                for n in c.body:
                    if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)) and n.name.startswith("test"):
                        seen.setdefault(n.name, n)
                    elif isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Name) and n.targets[0].id.startswith("test"):
                        seen.setdefault(n.targets[0].id, None)
            for method in sorted(k for k, v in seen.items() if v is not None):
                yield cls.name, method

    def extract(self, name, cls_name, method):
        """A standalone script for one test. Raises Excluded or CantExtract with the reason."""
        mod = self.mods["test." + name]
        cls = next(n for n in mod.tree.body if isinstance(n, ast.ClassDef) and n.name == cls_name)
        if cls_name in EXCLUDED_CLASSES:
            raise Excluded(EXCLUDED_CLASSES[cls_name])
        chain = self.chain(mod, cls)
        owner, fn = next((m, n) for m, c in chain for n in c.body
                         if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)) and n.name == method)
        for _, c in chain:
            check_decorators(c)
        check_decorators(fn)
        raw = owner.data[owner.starts[fn.lineno - 1]:owner.off(fn.end_lineno, fn.end_col_offset)].decode()
        for pattern, why in EXCLUDED_SOURCE:
            if re.search(pattern, raw):
                raise Excluded(why)
        if isinstance(fn, ast.AsyncFunctionDef):
            raise CantExtract("async test")
        if [a.arg for a in fn.args.args] != ["self"] or fn.args.vararg or fn.args.kwarg or fn.args.kwonlyargs:
            raise CantExtract("test method takes extra arguments")

        script = Script(self)
        body = owner.block(fn)
        names, attrs = uses(body)
        plain = (not self_used(body) and "super" not in names and not has_mangled(body)
                 and not any(n.name in ("setUp", "tearDown") for _, c in chain for n in c.body if isinstance(n, ast.FunctionDef)))
        if plain:
            text = dedent(body, fn.col_offset)
            text = re.sub(r"^def (\w+)\(self\)", r"def \1()", text, count=1)
            for n in names:
                script.need(owner, n)
            tail = f"{text}\n\n{method}()\n"
        else:
            tail = script.class_shape(chain, cls_name, method)
        header = f"# Extracted by tools/conformance.py from CPython {TAG} Lib/test/{name}.py, {cls_name}.{method}\n"
        header += "# (CPython's test suite is under the PSF licence: https://docs.python.org/3/license.html)\n"
        return script.finish(header, tail)


def check_decorators(node):
    for d in node.decorator_list:
        target = d.func if isinstance(d, ast.Call) else d
        last = target.attr if isinstance(target, ast.Attribute) else getattr(target, "id", "")
        if last in EXCLUDED_DECORATORS:
            raise Excluded(EXCLUDED_DECORATORS[last])
        raise CantExtract(f"decorator @{ast.unparse(target)}")


def self_used(text):
    """Whether the method reads its own `self`; a nested method's `self` is another object."""
    def walk(node):
        for c in ast.iter_child_nodes(node):
            if isinstance(c, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda)) and any(a.arg == "self" for a in c.args.args):
                continue
            if (isinstance(c, ast.Name) and c.id == "self") or walk(c):
                return True
        return False
    return walk(parse("class _:\n" + text).body[0].body[0])


def has_mangled(text):
    tree = parse("class _:\n" + text)
    for n in ast.walk(tree):
        name = n.id if isinstance(n, ast.Name) else n.attr if isinstance(n, ast.Attribute) else ""
        if name.startswith("__") and not name.endswith("__"):
            return True
    return False


class Script:
    """The definitions one test needs, gathered by following names from the test outward."""

    def __init__(self, universe):
        self.u, self.seen, self.nodes, self.lines = universe, set(), {}, set()

    def need(self, mod, name):
        if (mod.name, name) in self.seen:
            return
        self.seen.add((mod.name, name))
        kind = mod.imports.get(name)
        if name in mod.defs:
            for node in mod.defs[name]:
                self.include(mod, node)
        elif kind and kind[0] == "from" and kind[1] in self.u.mods:
            self.need(self.u.mods[kind[1]], kind[2])
        elif kind and kind[0] == "from":
            self.lines.add(f"from {kind[1]} import {kind[2]}" + (f" as {name}" if kind[2] != name else ""))
        elif kind:
            self.lines.add(f"import {kind[1]}" + (f" as {kind[2]}" if kind[2] else ""))
        else:
            for star in mod.stars:
                if star == "sys" and hasattr(sys, name):
                    self.lines.add(f"from sys import {name}")

    def include(self, mod, node):
        key = (self.u.order.index(mod.name), node.lineno)
        if key in self.nodes:
            return
        text = mod.block(node)
        self.nodes[key] = text
        for n in uses(text)[0]:
            self.need(mod, n)

    def class_shape(self, chain, leaf, method):
        """A test that needs self keeps its class, with only the members it reaches, and its bases."""
        picked, todo = {}, [method, "__init__", "setUp", "tearDown", "setUpClass", "tearDownClass"]
        done = set()
        while todo:
            name = todo.pop()
            if name in done:
                continue
            done.add(name)
            for i, (mod, c) in enumerate(chain):
                for node in c.body:
                    text = self.member(mod, node, name)
                    if text is None:
                        continue
                    picked.setdefault(i, {})[node.lineno] = text
                    names, attrs = uses(text)
                    for n in names:
                        self.need(mod, n)
                    todo += sorted(attrs | names)
        out, base = [], ""
        if any("super()" in t for ms in picked.values() for t in ms.values()):
            out.append("class _Case:\n    def setUp(self):\n        pass\n\n    def tearDown(self):\n        pass\n")
            base = "(_Case)"
        for i in range(len(chain) - 1, -1, -1):
            members = picked.get(i)
            if not members:
                continue
            title = leaf if i == 0 else f"_Base{i}"
            out.append(f"class {title}{base}:\n" + "\n\n".join(t for _, t in sorted(members.items())) + "\n")
            base = f"({title})"
        if 0 not in picked:
            out.append(f"class {leaf}{base}:\n    pass\n")
        has = lambda n: any(isinstance(x, ast.FunctionDef) and x.name == n for _, c in chain for x in c.body)
        calls = [f"{leaf}.setUpClass()"] if has("setUpClass") else []
        calls.append(f"_t = {leaf}()")
        calls += ["_t.setUp()"] if has("setUp") else []
        calls.append(f"_t.{method}()")
        calls += ["_t.tearDown()"] if has("tearDown") else []
        calls += [f"{leaf}.tearDownClass()"] if has("tearDownClass") else []
        return "\n".join(out) + "\n" + "\n".join(calls) + "\n"

    def member(self, mod, node, name):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) and node.name == name:
            return mod.block(node)
        if isinstance(node, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            if any(isinstance(t, ast.Name) and t.id == name for t in targets):
                return mod.block(node)
        if isinstance(node, ast.ImportFrom) and any((a.asname or a.name) == name for a in node.names):
            if node.module in self.u.mods:
                self.need(self.u.mods[node.module], name)
                return f"    {name} = {name}"
            self.lines.add(f"from {node.module} import {name}")
            return f"    {name} = {name}"
        return None

    def finish(self, header, tail):
        future = sorted(ln for ln in self.lines if ln.startswith("from __future__"))
        imports = sorted(self.lines - set(future))
        parts = [header + "\n".join(future + imports)] + [self.nodes[k] for k in sorted(self.nodes)] + [tail]
        script = "\n\n".join(p for p in parts if p.strip())
        validate(script)
        return script


def validate(script):
    """The last gate: only what the rewriting fully handled, and only safe imports, may run."""
    try:
        tree = parse(script)
    except SyntaxError as e:
        raise CantExtract(f"generated script does not parse: {e.msg}")
    for n in ast.walk(tree):
        if isinstance(n, ast.Import):
            roots = [a.name.split(".")[0] for a in n.names]
        elif isinstance(n, ast.ImportFrom):
            roots = [(n.module or "").split(".")[0]]
        else:
            roots = []
        for r in roots:
            if r not in ALLOWED_IMPORTS:
                raise CantExtract(f"imports {r}")
        if isinstance(n, ast.Name) and n.id in FORBIDDEN_NAMES and isinstance(n.ctx, ast.Load):
            raise CantExtract(f"uses {n.id}()")
        if isinstance(n, ast.Attribute) and isinstance(n.value, ast.Name) and n.value.id in RECEIVERS and n.attr.startswith("assert"):
            raise CantExtract(f"unittest API {n.attr}")
        f = n.func if isinstance(n, ast.Call) else None
        if isinstance(f, ast.Attribute) and isinstance(f.value, ast.Name) and f.value.id in RECEIVERS and f.attr in TEST_API:
            raise CantExtract(f"unittest API {f.attr}")


def cache_dir():
    base = os.environ.get("BREVI_CACHE")
    return (Path(base) if base else Path.home() / ".cache" / "brevi").resolve() / "conformance"


def load_sources(cache):
    """Every pinned file, from the cache if it is there, else from GitHub; each must match its hash."""
    out = {}
    for name, (path, sha) in SOURCES.items():
        f = cache / "cpython" / TAG / path
        data = f.read_bytes() if f.exists() else None
        if data is None or hashlib.sha256(data).hexdigest() != sha:
            print(f"fetching {path} at {TAG}", file=sys.stderr)
            with urllib.request.urlopen(RAW + path, timeout=60) as r:
                data = r.read()
            if hashlib.sha256(data).hexdigest() != sha:
                sys.exit(f"{path} at {TAG} does not match its pinned SHA-256; refusing to use it")
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_bytes(data)
        out[name] = data
    return out


def find_python(given):
    exe = given or shutil.which("python3.13") or "python3"
    try:
        version = subprocess.run([exe, "-c", "import sys; print(sys.version.split()[0]); print(sys.executable)"],
                                 capture_output=True, text=True, timeout=30).stdout.split()
    except OSError:
        version = []
    if not version or version[0] != PYTHON_VERSION:
        sys.exit(f"need CPython {PYTHON_VERSION} to match the pinned tests at {TAG}; {exe} is {version[:1] or 'not runnable'}. "
                 "Point --python at it (`uv python install 3.13.13`), or change TAG and SOURCES together.")
    return version[1]


Run = collections.namedtuple("Run", "rc out err note")


def run_limited(cmd, cwd, env, timeout, mem_mb, sandbox):
    """Runs a generated script or built program in its own process group: no network and writes only to
    its folder where sandbox-exec exists, CPU, output-size and (on Linux) address-space limits through
    ulimit, and on every platform a wall-clock timeout and a resident-memory watch."""
    limits = f"ulimit -c 0; ulimit -f {128 * 1024}; ulimit -t {int(timeout) + 5}; ulimit -v {mem_mb * 1024} 2>/dev/null;"
    prefix = []
    if sandbox:
        profile = ("(version 1)(allow default)(deny network*)(deny file-write*)"
                   f'(allow file-write* (subpath "{cwd}"))(allow file-write-data (literal "/dev/null"))')
        prefix = ["sandbox-exec", "-p", profile]
    shell = limits + " exec " + " ".join(shlex.quote(c) for c in prefix + cmd)
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        p = subprocess.Popen(["/bin/sh", "-c", shell], cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                             start_new_session=True)
        begun, note = time.monotonic(), ""
        while p.poll() is None:
            try:
                p.wait(0.5)
            except subprocess.TimeoutExpired:
                if time.monotonic() - begun > timeout:
                    note = "timeout"
                elif resident_mb(p.pid) > mem_mb:
                    note = "memory limit"
                if note:
                    try:
                        os.killpg(p.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    p.wait()
        out.seek(0)
        err.seek(0)
        return Run(p.returncode, out.read(), err.read(), note)


def resident_mb(pid):
    r = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)], capture_output=True, text=True)
    return int(r.stdout.strip() or 0) // 1024


def sandbox_works():
    if not shutil.which("sandbox-exec"):
        return False
    r = subprocess.run(["sandbox-exec", "-p", "(version 1)(allow default)(deny network*)", "/usr/bin/true"], capture_output=True)
    return r.returncode == 0


def run_env(folder, seed):
    return {"PATH": "/usr/bin:/bin", "HOME": str(folder), "TMPDIR": str(folder), "PYTHONUTF8": "1",
            "PYTHONHASHSEED": str(seed), "PYTHONDONTWRITEBYTECODE": "1"}


def reference(py, script, folder, seed, cfg):
    return run_limited([py, "-B", "-s", "-P", str(script)], str(folder), run_env(folder, seed), cfg.timeout, cfg.mem_mb, cfg.sandbox)


def last_line(data):
    lines = data.decode("utf-8", "replace").strip().splitlines()
    return lines[-1][:200] if lines else ""


def build(script, folder, mode, cfg):
    """(built binary or None, path report, why not built, build crashed or timed out)."""
    env = {k: v for k, v in os.environ.items() if k not in ("BREVI_DYN", "BREVI_PATH", "BREVI_NATIVE")}
    env.update(BREVI_PATH="1", BREVI_PYTHON=cfg.python)
    if mode == "dyn":
        env["BREVI_DYN"] = "1"
    exe = folder / f"prog-{mode}"
    try:
        r = subprocess.run([cfg.brevi, "build", script.name, "-o", str(exe)], cwd=folder, env=env, stdin=subprocess.DEVNULL,
                           capture_output=True, text=True, errors="replace", timeout=cfg.build_timeout)
    except subprocess.TimeoutExpired:
        return None, None, "build timed out", True
    path, why = None, ""
    for line in (r.stdout + "\n" + r.stderr).splitlines():
        if line.startswith("{"):
            try:
                j = json.loads(line)
            except ValueError:
                continue
            if "path" in j:
                path = j
            elif not why:
                why = str(j.get("msg") or j.get("why") or line)[:200]
    if r.returncode == 0:
        return exe, path, "", False
    crashed = r.returncode < 0 or "panicked at" in r.stderr
    return None, path, why or last_line((r.stdout + r.stderr).encode()), crashed


def first_diff(want, got):
    a, b = want.decode("utf-8", "replace").split("\n"), got.decode("utf-8", "replace").split("\n")
    for i, (x, y) in enumerate(zip(a, b)):
        if x != y:
            return f"line {i + 1}: expected {x[:120]!r}, got {y[:120]!r}"
    return f"expected {len(a) - 1} lines, got {len(b) - 1}"


def classify(want, built, got):
    """(category, detail). `want` is CPython's Run; `built` the build's (ok, why, crashed); `got` the program's Run."""
    ok, why, crashed = built
    if not ok:
        return ("crash_or_timeout", why) if crashed else ("refused", why)
    if got.note or got.rc < 0:
        return "crash_or_timeout", got.note or f"signal {signal.Signals(-got.rc).name}"
    if b"panicked at" in got.err:
        lines = got.err.decode("utf-8", "replace").splitlines()
        at = next(i for i, ln in enumerate(lines) if "panicked at" in ln)
        return "crash_or_timeout", "panic: " + "".join(lines[at + 1:at + 2])[:200]
    if got.rc != want.rc:
        return "wrong_exit", f"exit {got.rc}, expected {want.rc}: {last_line(got.err)}"
    if got.out != want.out:
        return "wrong_output", first_diff(want.out, got.out)
    return "matched", ""


def run_case(case, cfg):
    """Fills in the case's result for each mode."""
    folder = cfg.work / case["id"]
    shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True)
    script = folder / "case.py"
    script.write_text(case["script"], encoding="utf-8")
    a, b = (reference(cfg.python, script, folder, seed, cfg) for seed in (1, 2))
    if (a.rc, a.out) != (b.rc, b.out):
        return {**case, "category": "unstable", "detail": "CPython's output changes with PYTHONHASHSEED"}
    if a.rc != 0 or a.note:
        why = a.note or last_line(a.err)
        return {**case, "category": "not_extractable", "detail": f"generated script fails under CPython: {why}"}
    if NO_RAISE.encode() in a.out:
        return {**case, "category": "not_extractable", "detail": "assertRaises saw no exception in the extracted test"}
    modes = {}
    for mode in ("normal", "dyn"):
        exe, path, why, crashed = build(script, folder, mode, cfg)
        got = run_limited([str(exe)], str(folder), run_env(folder, 1), cfg.timeout, cfg.mem_mb, cfg.sandbox) if exe else None
        category, detail = classify(a, (exe is not None, why, crashed), got)
        if category == "wrong_output":
            # Two seeds can agree by luck on a small str set: try more before calling this Brevi's fault.
            for seed in (3, 4, 5):
                if reference(cfg.python, script, folder, seed, cfg).out != a.out:
                    return {**case, "category": "unstable", "detail": f"CPython's output changes with PYTHONHASHSEED={seed}"}
        modes[mode] = {"category": category, "path": (path or {}).get("path"), "detail": detail, "silent": not a.out,
                       "stderr": last_line(got.err) if got else ""}
    shutil.rmtree(folder, ignore_errors=True)
    return {**case, "modes": modes}


CATEGORIES = ["matched", "refused", "wrong_output", "wrong_exit", "crash_or_timeout", "not_extractable", "unstable", "excluded"]
# A native program that dies on one of these may be memory-unsafe, which SECURITY.md asks to report privately:
# the public report only counts them; the test names go to private_signals.json in the cache.
PRIVATE_SIGNALS = ("SIGSEGV", "SIGBUS", "SIGILL", "SIGABRT", "SIGFPE", "SIGTRAP")


def is_private(detail):
    return detail.startswith("signal ") and detail.split()[1] in PRIVATE_SIGNALS


def category_of(r, mode):
    return r["modes"][mode]["category"] if "modes" in r else r["category"]


def tally(results, mode):
    c = collections.Counter(category_of(r, mode) for r in results)
    counts = {k: c[k] for k in CATEGORIES}
    bridged = sum(1 for r in results if "modes" in r and r["modes"][mode]["category"] == "matched" and r["modes"][mode]["path"] == "pybridge")
    total = len(results) - counts["unstable"] - counts["excluded"]
    native = counts["matched"] - bridged
    silent = sum(1 for r in results if "modes" in r and r["modes"][mode]["category"] == "matched" and r["modes"][mode]["silent"])
    return {**counts, "total": len(results), "scored": total, "matched_through_cpython": bridged, "matched_silent": silent,
            "coverage": round(native / total, 4) if total else 0, "wrong": counts["wrong_output"] + counts["wrong_exit"]}


def bad_ids(results):
    return sorted(f"{r['id']} {mode}" for r in results if "modes" in r for mode in r["modes"] if r["modes"][mode]["category"] in BAD)


def read_floor():
    return json.loads(FLOOR.read_text()) if FLOOR.exists() else {}


def read_baseline():
    if not BASELINE.exists():
        return set()
    return {ln.strip() for ln in BASELINE.read_text().splitlines() if ln.strip() and not ln.startswith("#")}


def public_rows(results):
    rows = []
    for r in results:
        row = {"id": r["id"], "file": r["file"]}
        if "modes" not in r:
            rows.append({**row, "category": r["category"], "detail": r["detail"]})
            continue
        modes = {m: {k: v for k, v in x.items() if k in ("category", "path", "detail") and v} for m, x in r["modes"].items()}
        if any(is_private(x["detail"]) for x in r["modes"].values()):
            continue  # counted in the totals, named only in private_signals.json
        rows.append({**row, "modes": modes})
    return rows


def normalise(why):
    return re.sub(r"\d+", "N", re.sub(r"'[^']*'|\"[^\"]*\"|`[^`]*`", "X", why))[:90]


def markdown(rep, results):
    def table(mode):
        out = ["| File | matched | refused | wrong_output | wrong_exit | crash_or_timeout | not_extractable | unstable | excluded | total | coverage | wrong |",
               "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
        for name, t in list(rep["files"].items()) + [("**all**", rep["totals"])]:
            t = t[mode]
            cells = [t[k] for k in CATEGORIES] + [t["total"], f"{t['coverage'] * 100:.1f}%", t["wrong"]]
            out.append(f"| {name} | " + " | ".join(str(c) for c in cells) + " |")
        return out

    n, d = rep["totals"]["normal"], rep["totals"]["dyn"]
    lines = ["# CPython conformance", "",
             f"{rep['brevi'].capitalize()} against CPython {rep['python']} (tests from tag {rep['tag']}), {rep['date']}, {rep['platform']}.",
             f"Generated by `tools/conformance.py`; how to read and run it is in [bench/README.md](../README.md#cpython-conformance).", "",
             f"**Coverage {n['coverage'] * 100:.1f}%** ({n['matched'] - n['matched_through_cpython']} of {n['scored']} tests print exactly what CPython prints from a native build). "
             f"**Wrong: {n['wrong']}** built programs gave a different answer ({n['wrong_output']} output, {n['wrong_exit']} exit code).", "",
             f"Forced dynamic build (`BREVI_DYN=1`): coverage {d['coverage'] * 100:.1f}%, wrong {d['wrong']}.", "",
             "## Normal build", ""] + table("normal") + ["", "## Forced dynamic build", ""] + table("dyn")
    lines += ["", "## Paths taken", "", "| Build | " + " | ".join(rep["paths"]["normal"]) + " |",
              "| --- | " + " | ".join("---:" for _ in rep["paths"]["normal"]) + " |",
              "| normal | " + " | ".join(str(v) for v in rep["paths"]["normal"].values()) + " |"]
    lines += ["", f"{n['matched_through_cpython']} matched programs on the normal build ran library calls in CPython (`pybridge`). "
              "They stay in `matched` but do not count towards coverage.",
              f"{n['matched_silent']} matched programs print nothing: the test has no assertion, so a match means it built and exited 0."]
    lines += ["", "## Excluded (CPython internals)", ""]
    why = collections.Counter(r["detail"] for r in results if r.get("category") == "excluded")
    lines += [f"- {w}: {c}" for w, c in why.most_common()]
    for title, cat in (("Why tests are not extractable", "not_extractable"),):
        lines += ["", f"## {title}", ""]
        why = collections.Counter(normalise(r["detail"]) for r in results if r.get("category") == cat)
        lines += [f"- {c}: {w}" for w, c in why.most_common(25)]
    lines += ["", "## Top refusals (normal build)", ""]
    why = collections.Counter(normalise(r["modes"]["normal"]["detail"]) for r in results
                              if "modes" in r and r["modes"]["normal"]["category"] == "refused")
    lines += [f"- {c}: {w}" for w, c in why.most_common(25)]
    lines += ["", "## Wrong results", "", "Reproduce one with `python3 tools/conformance.py --show ID`.", ""]
    for r in results:
        for mode, x in r.get("modes", {}).items():
            if x["category"] in ("wrong_output", "wrong_exit"):
                lines.append(f"- `{r['id']}` ({mode}, {x['path']}): {x['category']}: {x['detail']}")
    lines += ["", "## Crashes and timeouts", ""]
    held = 0
    for r in results:
        for mode, x in r.get("modes", {}).items():
            if x["category"] == "crash_or_timeout":
                if is_private(x["detail"]):
                    held += 1
                else:
                    lines.append(f"- `{r['id']}` ({mode}): {x['detail']}")
    if held:
        lines.append(f"- {held} more ended on a signal; those are looked at privately first (see SECURITY.md).")
    return "\n".join(lines) + "\n"


def extract_all(universe, names, pattern):
    cases = []
    for name in names:
        for cls, method in universe.tests(name):
            cid = f"{name}.{cls}.{method}"
            if pattern and not re.search(pattern, cid):
                continue
            case = {"id": cid, "file": name}
            try:
                case["script"] = universe.extract(name, cls, method)
            except Excluded as e:
                case.update(category="excluded", detail=str(e))
            except CantExtract as e:
                case.update(category="not_extractable", detail=str(e))
            cases.append(case)
    return cases


def show(cases, cid, cfg):
    case = next((c for c in cases if c["id"] == cid), None)
    if case is None:
        sys.exit(f"no test {cid}")
    if "script" not in case:
        sys.exit(f"{cid}: {case['category']}: {case['detail']}")
    print(case["script"])
    print(f"# Save this as case.py, then:\n#   {cfg.python} case.py\n#   BREVI_PATH=1 {cfg.brevi} build case.py -o prog && ./prog\n"
          "#   BREVI_PATH=1 BREVI_DYN=1 ... the same, for the forced dynamic build")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--files", default=",".join(SCOPE), help="comma-separated test files (default: %(default)s)")
    ap.add_argument("--filter", help="only test ids matching this regular expression")
    ap.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 1), help="parallel tests (default: at most 4)")
    ap.add_argument("--timeout", type=float, default=20, help="seconds per program run (default: %(default)s)")
    ap.add_argument("--build-timeout", type=float, default=300, help="seconds per build (default: %(default)s)")
    ap.add_argument("--mem-mb", type=int, default=2048, help="memory limit per program run (default: %(default)s)")
    ap.add_argument("--python", help=f"the CPython {PYTHON_VERSION} to compare with (default: python3.13 on PATH)")
    ap.add_argument("--out", type=Path, help="folder for report.json and report.md (default: the cache)")
    ap.add_argument("--check", action="store_true", help="exit 1 on a wrong result not in the baseline")
    ap.add_argument("--write-baseline", action="store_true", help=f"replace {BASELINE.relative_to(ROOT)} with this run's wrong results")
    ap.add_argument("--show", metavar="ID", help="print one test's generated script and stop")
    cfg = ap.parse_args()
    names = cfg.files.split(",")
    unknown = [n for n in names if "test." + n not in SOURCES]
    if unknown:
        sys.exit(f"not pinned in SOURCES: {', '.join(unknown)}")
    cache = cache_dir()
    universe = Universe(load_sources(cache))
    cfg.python = find_python(cfg.python)
    cfg.brevi = str(Path(os.environ.get("BREVI", ROOT / "target" / "release" / "brevi")).resolve())
    cases = extract_all(universe, names, cfg.filter)
    if cfg.show:
        return show(cases, cfg.show, cfg)
    if not Path(cfg.brevi).exists():
        sys.exit(f"no compiler at {cfg.brevi}: run cargo build --release")
    cfg.work, cfg.sandbox = cache / "work", sandbox_works()
    cfg.work.mkdir(parents=True, exist_ok=True)
    runnable = [c for c in cases if "script" in c]
    print(f"{len(cases)} tests, {len(runnable)} extracted, jobs {cfg.jobs}", file=sys.stderr)
    with cf.ThreadPoolExecutor(cfg.jobs) as ex:
        futures = {c["id"]: ex.submit(run_case, c, cfg) for c in runnable}
        done = 0
        for f in cf.as_completed(futures.values()):
            done += 1
            if done % 50 == 0:
                print(f"  {done}/{len(runnable)}", file=sys.stderr)
    results = [futures[c["id"]].result() if "script" in c else c for c in cases]
    for r in results:
        r.pop("script", None)
    version = subprocess.run([cfg.brevi, "--version"], capture_output=True, text=True).stdout.strip()
    rep = {"brevi": version, "python": PYTHON_VERSION, "tag": TAG, "date": time.strftime("%Y-%m-%d"), "platform": sys.platform,
           "sandboxed": cfg.sandbox, "timeout_s": cfg.timeout, "mem_mb": cfg.mem_mb,
           "totals": {m: tally(results, m) for m in ("normal", "dyn")},
           "files": {n: {m: tally([r for r in results if r["file"] == n], m) for m in ("normal", "dyn")} for n in names},
           "paths": {m: dict(collections.Counter(r["modes"][m]["path"] or "refused" for r in results if "modes" in r).most_common()) for m in ("normal", "dyn")}}
    out = cfg.out or cache
    out.mkdir(parents=True, exist_ok=True)
    (out / "report.json").write_text(json.dumps({**rep, "tests": public_rows(results)}, indent=0) + "\n")
    (out / "report.md").write_text(markdown(rep, results))
    private = [{"id": r["id"], "mode": m, "detail": x["detail"], "stderr": x["stderr"]} for r in results
               for m, x in r.get("modes", {}).items() if is_private(x["detail"])]
    (cache / "private_signals.json").write_text(json.dumps(private, indent=1) + "\n")
    n = rep["totals"]["normal"]
    print(f"coverage {n['coverage'] * 100:.1f}%, wrong {n['wrong']} ({n['wrong_output']} output, {n['wrong_exit']} exit); report in {out}")
    bad = set(bad_ids(results))
    matched = {m: rep["totals"][m]["matched"] for m in ("normal", "dyn")}
    if cfg.write_baseline:
        BASELINE.write_text("# Known wrong results and crashes of tools/conformance.py: `test id` then normal or dyn. `--check` fails on a\n"
                            "# line not here, and on a line here that is no longer wrong, so this list only shrinks. Remove a line when the\n"
                            "# bug is fixed; add one only with `--write-baseline` after reading the result.\n"
                            + "".join(w + "\n" for w in sorted(bad)))
        FLOOR.write_text(json.dumps({"matched": matched, "note": "tools/conformance.py --check fails below these; --write-baseline raises them"}, indent=1) + "\n")
    if cfg.check:
        # The ratchet: the score only goes up, and the list of known-wrong results only shrinks.
        known, floor, failed = read_baseline(), read_floor().get("matched", {}), False
        ran = {r["id"] for r in results}  # --filter and --files say nothing about the tests they leave out
        new, gone = sorted(bad - known), sorted(k for k in known - bad if k.split()[0] in ran)
        if new:
            print(f"{len(new)} NEW wrong results or crashes (fix them, or read each and add it with --write-baseline):\n  " + "\n  ".join(new))
            failed = True
        if gone:
            print(f"{len(gone)} baseline lines are no longer wrong; remove them from {BASELINE.relative_to(ROOT)}:\n  " + "\n  ".join(gone))
            failed = True
        if not cfg.filter and cfg.files == ",".join(SCOPE):
            for m, n in matched.items():
                if m in floor and n < floor[m]:
                    print(f"{m} build: {n} tests match, below the floor of {floor[m]}; find the regression (or lower the floor on purpose)")
                    failed = True
                elif m in floor and n > floor[m]:
                    print(f"{m} build: {n} tests match, above the floor of {floor[m]}; raise it with --write-baseline")
        if failed:
            sys.exit(1)
        print(f"ratchet holds: no new wrong results ({len(bad)} known), matches {matched}")


if __name__ == "__main__":
    main()
