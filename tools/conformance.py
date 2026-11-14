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
  python3 tools/conformance.py --write-baseline    make the baseline the wrong results and crashes of this run, and lower
                                                   the floor to this run's match counts where they are below it
  python3 tools/conformance.py --show ID           print one test's generated script and how to reproduce it

Categories: matched, refused, by_design, string_model, cpython_private, wrong_output, wrong_exit, crash_or_timeout, not_extractable, unstable,
and excluded (CPython internals, each with a reason). by_design is a test the build refuses because it runs a string
as code (compile/exec/eval), which a sandboxed native binary refuses on purpose; each one is listed by name in
compat/conformance_by_design.txt and printed beside the score. string_model is a listed test (compat/
conformance_needs_string_model.txt, each with an issue) that stops in both builds because it needs a str holding a lone
surrogate; it is printed beside the score and counts as not matched. cpython_private is a listed test (compat/
conformance_cpython_private.txt, each with an issue) that both builds refuse because it calls a CPython-private module
helper (re._compile, functools._c3_mro); it is printed beside the score and counts as not matched. A test that calls
self.skipTest() under the reference CPython is excluded as skipped by CPython, like a skip decorator.
unstable is a test whose CPython output depends on PYTHONHASHSEED, and only a reproduced difference counts: each seed's
output is settled by a second run at that seed, and a CPython run that timed out or died is retried, then reported as
not_extractable, never as unstable (rule 1: seeds 1 and 2; rule 2: seeds 3 to 5, tried for a wrong result).
coverage = matched / (all - unstable - excluded - by_design); wrong = wrong_output + wrong_exit among programs that built.

The test files are fetched once from the pinned tag into a cache outside git (not vendored: nothing of
CPython's lands in this repository, so no PSF notice has to travel with it, and a new pin is one edit).
Every file is checked against its SHA-256, so later runs need no network. Fetched files are only parsed
with `ast`, never imported or run; only generated scripts run, under a timeout and a memory limit.
Env: BREVI (default target/release/brevi), BREVI_CACHE (the cache is its conformance/ folder, else
~/.cache/brevi/conformance).
"""
import argparse, ast, collections, concurrent.futures as cf, hashlib, json, os, re, shlex, shutil, signal
import functools, importlib, subprocess, symtable, sys, tempfile, time, urllib.request, warnings
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASELINE = ROOT / "compat" / "conformance_known_wrong"  # one file per known-wrong entry (see read_baseline)
FLOOR = ROOT / "compat" / "conformance_floor.json"  # the match counts a run may not fall below
BY_DESIGN_FILE = ROOT / "compat" / "conformance_by_design.txt"  # reviewed: tests refused on purpose, shared by both slices
# The refusal that makes a test by-design: the dynamic build's own reason (the normal build's names the typed path it
# tried first). Only the plain "X() is not supported" of a string run as code; eval() of a literal is supported.
BY_DESIGN_REFUSAL = re.compile(r"^(compile|exec|eval)\(\) is not supported in dynamic builds$")
# Tests that stop because they need the string model (a str holding a lone surrogate, which a Rust string cannot):
# reviewed, each tied to an open issue, printed beside every score. Matched only by this exact run-time stop, in both
# builds, and only for a listed id: no other run-time refusal counts, and the tests stay in the denominator.
STRING_MODEL_FILE = ROOT / "compat" / "conformance_needs_string_model.txt"
# The stop is the runtime's gate (S::text() in src/dynpy/rt_core.rs): a str holding one reached code not yet written
# for it (handoffs/string-model-design.md).
STRING_MODEL_STOP = re.compile(r"^exit 1, expected 0: NotImplementedError: brevi: this operation on a str with a lone surrogate "
                               r"\(U\+D800\.\.U\+DFFF\) is not supported in dynamic builds yet \(runtime line \d+\)$")
# Tests that call a CPython-private helper (a module attribute starting with `_`, such as re._compile or
# functools._c3_mro) that Brevi refuses: reviewed, each tied to an open issue, printed beside every score. Matched only
# when the dynamic build refuses exactly such a private name, the normal build refuses too, and the id is listed; the
# tests stay in the denominator as not matched.
PRIVATE_FILE = ROOT / "compat" / "conformance_cpython_private.txt"
PRIVATE_REFUSAL = re.compile(r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*\._[A-Za-z0-9]\w* is not supported in dynamic builds$")
BAD = ("wrong_output", "wrong_exit", "crash_or_timeout")  # built, but did not do what CPython does
TAG = "v3.13.13"
PYTHON_VERSION = "3.13.13"
RAW = "https://raw.githubusercontent.com/python/cpython/" + TAG + "/Lib/"
SCOPE = ["test_grammar", "test_scope", "test_int", "test_list", "test_dict", "test_contains", "test_binop", "test_richcmp", "test_set",
         "test_patma"]
# The second, separately published number: the stdlib modules agents use most (`--slice stdlib`), with its own
# floor and known-wrong list so neither slice's ratchet hides the other's.
SLICES = {
    "core": (SCOPE, BASELINE, FLOOR),
    "stdlib": (["test_itertools", "test_collections", "test_re", "test_textwrap", "test_functools", "test_string",
               "test_graphlib", "test_colorsys", "test_deque", "test_dataclasses"],
               ROOT / "compat" / "conformance_stdlib_known_wrong", ROOT / "compat" / "conformance_stdlib_floor.json"),
}
# Module name -> (path under Lib/, SHA-256). The helpers are only read for the definitions tests use.
SOURCES = {
    "test.support": ("test/support/__init__.py", "79347b3f68109a2a1315bba9c7769acc631908ade382ddeef473bfb72aa75ca9"),
    "test.support.warnings_helper": ("test/support/warnings_helper.py", "515c10201568a2fcb868d1c34ffe9d7f04e3f617a2cb61d587167359a83b09ec"),
    "test.support.numbers": ("test/support/numbers.py", "fd9c8f35ef65c32612599a89a3ff8fe320268bd139a1c1a773cdfdd44096202c"),
    "test.seq_tests": ("test/seq_tests.py", "030efe9c5f9b5a7cbe06ab7a5d590e9ec7a6b2b54a22b92183f95b8503f6c19c"),
    "test.list_tests": ("test/list_tests.py", "229ac95acf5e22e24795a69a464da4bde446176df297d1782f4c87e535d80f2f"),
    "test.mapping_tests": ("test/mapping_tests.py", "a9553128a09fc2190fbf085ce19eae3edf7f1b358765dd0ff7880037a9a8460a"),
    "test.test_grammar": ("test/test_grammar.py", "301bccc2db95dc3147d0ef4225a02bfe4eeb73fab62138863594b9851debd993"),
    "test.test_scope": ("test/test_scope.py", "ed697656916d59ca03268751a09f3f552a4bda72ec7ff6c0933602b49c994d62"),
    "test.test_int": ("test/test_int.py", "2d947888f306e945ebc12c6817691330528c0bc7a13330b942c9af97fe3075d8"),
    "test.test_list": ("test/test_list.py", "088eb797e98cbbf6374bd1e646e4d8c6cecfc4ca114873653336e661e7938887"),
    "test.test_dict": ("test/test_dict.py", "009c957818d2e4c13b5c1e5a2bf90c7122ad77ac17362f8ef3ec6b8f6bf61f06"),
    "test.test_set": ("test/test_set.py", "05f3ed822ab460c2efa96898cca6d9df4a2eac1833664c579450600eb2ddcd22"),
    "test.test_itertools": ("test/test_itertools.py", "b356e71312eaac265c7104c32176c5150ab06433754bcb25d6e0013f321ca1c9"),
    "test.test_collections": ("test/test_collections.py", "d9db27a28044d2d0cd24a54299d87e0f5c171f46115b91087163726ccab14bfa"),
    "test.test_re": ("test/test_re.py", "fe86c393ae0a94bd9b5e4577d08b99fc60d825e04f4440486562868fb3d3e620"),
    "test.test_textwrap": ("test/test_textwrap.py", "1f16be769aa2aa32028b821bca3e90c3bdfabb37ffdbafa20db09ee627009214"),
    "test.test_functools": ("test/test_functools.py", "ec00d729ddde2de6412237b3f8f4196bc0777ce9d0bf99fd69071fb44ce72fd5"),
    "test.test_string": ("test/test_string.py", "defe67cc39c2a4737f1dad3a6c7877cb4eb96df6c331966214b77548f6b2c6a6"),
    "test.test_contains": ("test/test_contains.py", "34916d854f7f04c3189cd7d8e04177bca3c55bf1acdccf88beac501ff24239f4"),
    "test.test_binop": ("test/test_binop.py", "85af7379285b3787d61be4926d38fa188d7a7f11413398fd6364886208f4fd3f"),
    "test.test_richcmp": ("test/test_richcmp.py", "87ffd49f08a617224cdbb84f0eeb7cef643f3cd918090c606a311a25d929d135"),
    "test.test_patma": ("test/test_patma.py", "651dcbe1421d8f6fda77f6319cedd3c2ca67193de805d08cadd22a3e9efc48ed"),
    "test.test_graphlib": ("test/test_graphlib.py", "87cde0e38dc1901e938356f4627f6842ff0cf9a74582edc1f22868b295944b3e"),
    "test.test_colorsys": ("test/test_colorsys.py", "f7ea1907f751f0201ea69a0bc368cdb2b24e7c7f958d8df659a8ff84142bfb93"),
    "test.test_deque": ("test/test_deque.py", "15578b602865dfa5c80baa64006d64c5875765aab457d5b4bedd73a5e8436d73"),
    "test.test_dataclasses": ("test/test_dataclasses/__init__.py", "eb6d80602be85858a9a5f559c6f9fe7949b48972df4024aff8c825fcf8548cb4"),
    "test.re_tests": ("test/re_tests.py", "33033dc23d20a6924391819395e7bdd3c16d11cf3bb79eedb6298e79f65bb4e8"),  # data for test_re.ExternalTests
}
# Modules some tests import as modules (test_grammar reads their __annotations__): copied as they are next to the
# generated script, so the reference and the build import the same pinned files. Never parsed for definitions.
COMPANIONS = {
    "test": ("test/__init__.py", "836cdb388117cf81e78d9fa2a141cca1b14b0179733322e710067749a1b16fe9"),
    "test.typinganndata": ("test/typinganndata/__init__.py", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
    "test.typinganndata.ann_module": ("test/typinganndata/ann_module.py", "14c92d11f7e53a1d315e9125458a68105097d152dbee27cd063c9f6664c7453c"),
    "test.typinganndata.ann_module2": ("test/typinganndata/ann_module2.py", "2f1214af1113c659b37ff02aa9727f3341812e066c82524c471e4325bcde6f72"),
    "test.typinganndata.ann_module3": ("test/typinganndata/ann_module3.py", "c72c7dfa54f5af1bb9ad263964adf130597666ae1e5cd125f5a435b565d6c15f"),
}

# A generated script may import only these: nothing that reaches files, processes or the network.
ALLOWED_IMPORTS = {
    "__future__", "abc", "array", "bisect", "builtins", "cmath", "collections", "colorsys", "contextlib", "copy",
    "dataclasses", "decimal", "enum", "fractions", "functools", "gc", "graphlib", "heapq", "inspect", "itertools", "json",
    "math", "numbers", "locale", "operator", "pickle", "random", "re", "string", "struct", "sys", "textwrap", "threading", "time", "types",
    "typing", "unicodedata", "warnings", "weakref",
}
# Modules a generated script may use only in part: each name it reads from them must be in the set (io.open or
# io.FileIO would reach files; unittest is allowed only for unittest.mock).
RESTRICTED_IMPORTS = {"io": {"StringIO", "BytesIO"}, "unittest": {"mock"}}
@functools.lru_cache(None)
def star_names(module):
    """The names `from module import *` binds, for an allowed stdlib module."""
    m = importlib.import_module(module)
    return set(getattr(m, "__all__", None) or (n for n in vars(m) if not n.startswith("_")))


FORBIDDEN_NAMES = {"open", "__import__", "breakpoint", "input"}
# Tests of CPython itself, not of the language: left out of the total, each with its reason.
EXCLUDED_DECORATORS = {
    "cpython_only": "marked cpython_only",
    "suppress_immortalization": "tests CPython's immortal objects",
    "requires_resource": "needs an opt-in test resource",
    "bigmemtest": "needs gigabytes of memory",
    "bigaddrspacetest": "needs gigabytes of memory",  # skips itself unless run with -M, which regrtest does not pass
}
# Names a skip condition may read, and what each is under the reference run (no -O, a normal build). Anything else
# in a condition leaves the test not_extractable: only a condition we can evaluate the way CPython would is decided.
CONDITION_PRELUDE = """import os, sys, re, sysconfig
is_emscripten = sys.platform == "emscripten"
is_wasi = sys.platform == "wasi"
class support:
    is_wasi = is_wasi
try:
    import _multiprocessing
except ImportError:
    multiprocessing = None
else:
    import multiprocessing
"""
CONDITION_BINDINGS = {"sys": ("import", "sys", None), "re": ("import", "re", None), "support": ("from", "test", "support"),
                      "is_emscripten": ("from", "test.support", "is_emscripten"), "is_wasi": ("from", "test.support", "is_wasi")}
CONDITION_NAMES = {"sys", "re", "hasattr", "is_emscripten", "is_wasi", "support", "multiprocessing", "__debug__"}
# support.requires_docstrings is skipUnless(HAVE_DOCSTRINGS, ...); with no -O, HAVE_DOCSTRINGS is `not MISSING_C_DOCSTRINGS`
# (Lib/test/support/__init__.py: MISSING_C_DOCSTRINGS = check_impl_detail() and platform != 'win32' and no WITH_DOC_STRINGS).
DOCSTRINGS_CONDITION = 'not (sys.platform != "win32" and not sysconfig.get_config_var("WITH_DOC_STRINGS"))'
# support.MISSING_C_DOCSTRINGS itself, for `@unittest.skipIf(support.MISSING_C_DOCSTRINGS, ...)`
MISSING_DOCSTRINGS_CONDITION = f"not ({DOCSTRINGS_CONDITION})"
# threading_helper.requires_working_threading() is skipUnless(can_start_thread); support._can_start_thread() is False
# only on WASI and on Emscripten without pthreads.
THREADS_CONDITION = 'sys.platform != "wasi" and (sys.platform != "emscripten" or sys._emscripten_info.pthreads)'
# support.skip_on_s390x is skipIf(is_s390x), with is_s390x = hasattr(os, 'uname') and os.uname().machine == 's390x'
S390X_CONDITION = 'hasattr(os, "uname") and os.uname().machine == "s390x"'
# What a decorator that wraps a test does, as helper code the generated script carries when it is used. All of it
# mirrors CPython's definitions: pickle_deprecated is in Lib/test/test_itertools.py (runs the test three times), the
# others are unittest's assertWarns and Lib/test/support/warnings_helper.py's ignore_warnings.
HELPERS = {
    "_conf_Warns": """class _conf_Warns:
    def __init__(self, category, regex=None):
        self.category, self.regex = category, regex

    def __enter__(self):
        self.ctx = warnings.catch_warnings(record=True)
        self.warnings = self.ctx.__enter__()
        warnings.simplefilter("always")
        return self

    def __exit__(self, exc_type, exc, tb):
        self.ctx.__exit__(exc_type, exc, tb)
        if exc_type is not None:
            return False
        self.warning = self.filename = self.lineno = None
        for m in self.warnings:
            if not isinstance(m.message, self.category):
                continue
            if self.regex is not None and not re.search(self.regex, str(m.message)):
                continue
            self.warning, self.filename, self.lineno = m.message, m.filename, m.lineno
            break
        print('warns', self.warning is not None, sorted(m.category.__name__ for m in self.warnings))
        return False""",
    "_conf_pickle_deprecated": """def _conf_pickle_deprecated(testfunc):
    def inner():
        with _conf_Warns(DeprecationWarning):
            testfunc()
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", category=DeprecationWarning)
            testfunc()
        with warnings.catch_warnings():
            warnings.simplefilter("error", category=DeprecationWarning)
            try:
                testfunc()
            except (DeprecationWarning, AssertionError, SystemError) as _e:
                print('raises', type(_e).__name__)
            else:
                print('NO-RAISE')
    return inner""",
    "_conf_ignore_warnings": """def _conf_ignore_warnings(category):
    def decorator(test):
        def wrapper():
            with warnings.catch_warnings():
                warnings.simplefilter('ignore', category=category)
                return test()
        return wrapper
    return decorator""",
    "_conf_search": """def _conf_search(text, regex):
    print(repr(text))
    print(re.search(regex, text) is not None)""",
    # unittest's assertRaises as a context manager, for a `with` that holds more than one manager
    # (`with self.assertRaises(E), self.assertWarns(W):`), where the one-manager try/except rewrite cannot reach
    "_conf_Raises": """class _conf_Raises:
    def __init__(self, category, regex=None):
        self.category, self.regex = category, regex

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        if exc_type is None:
            print('NO-RAISE')
            return False
        if not issubclass(exc_type, self.category):
            return False
        print('raises', exc_type.__name__, *([str(exc)] if self.regex is not None else []))
        return True""",
    # self.skipTest() inside a test: the reference run prints the marker, and the scorer then leaves the test out
    # as skipped by CPython (run_case), the same as a skip decorator that is evaluated before the run
    "_conf_Skip": """class _conf_Skip(Exception):
    pass""",
    "_conf_cleanups": """_conf_cleanups = []""",
    "_conf_run": """def _conf_run(test):
    try:
        test()
    except _conf_Skip as _s:
        print('""" + "_conf_skipped:" + """', _s)""",
    # Lib/test/support/threading_helper.py's start_threads, without its verbose prints and faulthandler dump
    "_conf_start_threads": """@contextlib.contextmanager
def _conf_start_threads(threads, unlock=None):
    threads = list(threads)
    started = []
    try:
        for t in threads:
            t.start()
            started.append(t)
        yield
    finally:
        try:
            if unlock:
                unlock()
            endtime = time.monotonic()
            for timeout in range(1, 16):
                endtime += 60
                for t in started:
                    t.join(max(endtime - time.monotonic(), 0.01))
                started = [t for t in started if t.is_alive()]
                if not started:
                    break
        finally:
            started = [t for t in started if t.is_alive()]
            if started:
                raise AssertionError('Unable to join %d threads' % len(started))""",
    # The base of a test whose assertions sit inside a string it runs as code (exec), where they cannot be rewritten
    "_conf_TestCase": """class _conf_TestCase:
    def setUp(self):
        pass

    def tearDown(self):
        pass

    def assertEqual(self, first, second, msg=None):
        print(repr(first))
        print(repr(second))

    def assertTrue(self, expr, msg=None):
        print(repr(bool(expr)))

    def assertFalse(self, expr, msg=None):
        print(repr(bool(expr)))

    def fail(self, msg=None):
        raise AssertionError(msg)""",
}
EXEC_SHIM = {"assertEqual", "assertTrue", "assertFalse", "fail"}  # the methods _conf_TestCase has
SKIPPED = b"_conf_skipped:"  # what _conf_run prints when the test called skipTest
HELPER_IMPORTS = {"_conf_Warns": ["import re", "import warnings"], "_conf_pickle_deprecated": ["import warnings"],
                  "_conf_ignore_warnings": ["import warnings"], "_conf_search": ["import re"], "_conf_Raises": [],
                  "_conf_Skip": [], "_conf_cleanups": [], "_conf_run": [], "_conf_TestCase": [],
                  "_conf_start_threads": ["import contextlib", "import time"]}
HELPER_NEEDS = {"_conf_pickle_deprecated": ["_conf_Warns"], "_conf_run": ["_conf_Skip"]}
EXCLUDED_CLASSES = {
    "PyLongModuleTests": "tests _pylong, a private CPython module",
    "CAPITest": "tests the C API",
}
# (file, method): tests of one CPython C implementation detail, excluded wherever the method is (C and subclass classes)
_ODICT_DICT_METHODS = ("dict's own method called on an OrderedDict: tests that CPython's C OrderedDict survives its "
                       "linked list going out of step with the dict table (Brevi refuses the call with a clear error)")
EXCLUDED_TESTS = {("test_ordered_dict", m): _ODICT_DICT_METHODS for m in (
    "test_dict_setitem", "test_dict_delitem", "test_dict_clear", "test_dict_pop", "test_dict_popitem",
    "test_dict_setdefault", "test_dict_update")}
EXCLUDED_SOURCE = [
    (r"\b_test(capi|internalcapi|limitedcapi)\b", "tests the C API"),
    (r"\bsys\.getrefcount\b|\bcheck_free_after_iterating\b", "depends on reference counts"),
    (r"\bsys\.getsizeof\b|\b__sizeof__\b", "depends on object sizes"),
    (r"\bgc\.is_tracked\b|\bcheck_impl_detail\b|\brun_in_subinterp\b", "tests CPython internals"),
    (r"\bCPUStopwatch\b", "measures CPU time"),
    (r"\bassert_python_(ok|failure)\b|\bspawn_python\b", "runs a fresh CPython in a subprocess (a CPython crash regression)"),
    (r"\bmultiprocessing\.Process\b", "starts a child process"),
    (r"\bsre_(compile|constants|parse)\b", "tests CPython's deprecated sre_* modules"),
]
# Tests whose CPython output comes from unseeded random numbers (test_set's *Mutating* classes print a line only when
# a random __eq__ happens to raise RuntimeError): two CPython runs agree by luck most of the time, so such a test
# flipped between matched and unstable from run to run and moved the floor. They are always classed unstable, but
# both builds still run them, so a crash, a wrong exit code or a refusal still counts.
RANDOM_OUTPUT = re.compile(r"^test_set\.Test(BinaryOps|Methods)Mutating_\w+\.test_\w+_with_mutation$")
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


def fresh_module(node):
    """(name, module) for `name = import_helper.import_fresh_module('module', ...)`, else None."""
    if not (isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name)):
        return None
    v = node.value
    if (isinstance(v, ast.Call) and isinstance(v.func, ast.Attribute) and v.func.attr == "import_fresh_module"
            and isinstance(v.func.value, ast.Name) and v.func.value.id == "import_helper"
            and v.args and isinstance(v.args[0], ast.Constant) and isinstance(v.args[0].value, str)):
        return node.targets[0].id, v.args[0].value, any(k.arg == "blocked" for k in v.keywords)
    return None


class Mod:
    """One fetched source file: its top-level definitions, and the edits that rewrite its assertions.

    Edits are made on the source text, not the tree, because ast.unparse would turn 0xff into 255 and
    join implicit string concatenations, which is what test_grammar is testing."""

    def __init__(self, name, data):
        self.name, self.data, self.tree = name, data, parse(data)
        self.starts = [0]
        for line in data.split(b"\n"):
            self.starts.append(self.starts[-1] + len(line) + 1)
        self.edits, self.defs, self.imports, self.stars, self.py_copies, self.fresh = [], {}, {}, [], set(), set()
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
        elif fresh := fresh_module(node):
            # `py_functools = import_helper.import_fresh_module('functools', blocked=['_functools'])`: CPython's tests
            # run some classes against both its C and its pure-Python copy of a module. Brevi has one copy, so each
            # variant is the module itself, compared with what the reference CPython prints for its default copy.
            self.imports[fresh[0]] = ("import", fresh[1], fresh[0])
            self.fresh.add(fresh[0])
            if fresh[2]:
                self.py_copies.add(fresh[0])
        elif isinstance(node, ast.If) and isinstance(node.test, ast.Name) and node.test.id in self.fresh and not node.orelse:
            # `if c_functools: class CPartialSubclass(c_functools.partial): ...`: the copy always exists, so the
            # block defines what it defines; a test that needs one of its names takes the whole block
            for sub in ast.walk(node):
                if isinstance(sub, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) or (isinstance(sub, ast.Name) and isinstance(sub.ctx, ast.Store)):
                    self.defs.setdefault(sub.name if not isinstance(sub, ast.Name) else sub.id, []).append(node)
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

    def bare(self, node):
        """Like block, without the decorators."""
        return self.text(self.starts[node.lineno - 1], self.span(node)[1])

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
        self.m, self.helpers, self.caught, self.alias = mod, helper_modules, set(), {}

    def visit_FunctionDef(self, node):
        saved, self.caught, saved_alias = self.caught, set(), self.alias
        self.alias = self.find_aliases(node)
        self.generic_visit(node)
        self.caught, self.alias = saved, saved_alias

    def find_aliases(self, fn):
        """`eq = self.assertEqual` inside a test: calls of `eq` are the assertion, if that is all `eq` is used for."""
        found, uses_of = {}, collections.Counter()
        for sub in ast.walk(fn):
            if (isinstance(sub, ast.Assign) and len(sub.targets) == 1 and isinstance(sub.targets[0], ast.Name)
                    and isinstance(sub.value, ast.Attribute) and isinstance(sub.value.value, ast.Name)
                    and sub.value.value.id in RECEIVERS and sub.value.attr.startswith("assert")):
                found.setdefault(sub.targets[0].id, []).append(sub)
            elif isinstance(sub, ast.Name):
                uses_of[sub.id] += 1  # every mention, stores included
        calls = collections.Counter()
        for sub in ast.walk(fn):
            if isinstance(sub, ast.Expr):
                v = sub.value.elts[0] if isinstance(sub.value, ast.Tuple) and len(sub.value.elts) == 1 else sub.value
                if isinstance(v, ast.Call) and isinstance(v.func, ast.Name):
                    calls[v.func.id] += 1
        out = {}
        for name, assigns in found.items():
            # one assignment, and no mention of the name beyond it and calls written as statements
            if len(assigns) == 1 and uses_of[name] == 1 + calls[name]:
                out[name] = assigns[0]
        return out

    def visit_Attribute(self, node):
        self.generic_visit(node)
        v = node.value
        if not isinstance(v, ast.Name):
            return
        if node.attr == "exception" and v.id in self.caught:
            self.m.edit(*self.m.span(node), v.id + "_exception")
        kind = self.m.imports.get(v.id)
        if kind == ("from", "test.support", "threading_helper") and node.attr == "start_threads":
            self.m.edit(*self.m.span(node), "_conf_start_threads")
            return
        if kind and kind[0] == "from" and f"{kind[1]}.{kind[2]}" in self.helpers:
            self.m.imports.setdefault(node.attr, ("from", f"{kind[1]}.{kind[2]}", node.attr))
            self.m.edit(*self.m.span(node), node.attr)

    def visit_ImportFrom(self, node):
        # `from test.re_tests import tests` inside a test: the names come from that fetched module like any other
        if node.col_offset and node.module in self.helpers and node.level == 0 and self.m.alone(node):
            for a in node.names:
                self.m.imports.setdefault(a.asname or a.name, ("from", node.module, a.name))
            self.m.edit(*self.m.span(node), "pass")

    def visit_Assign(self, node):
        self.generic_visit(node)
        if any(a is node for a in self.alias.values()):
            self.m.edit(*self.m.span(node), "pass")
        fresh = fresh_module(node)
        if fresh and node.col_offset and fresh[1] in COMPANIONS and len(node.value.args) == 1 and not node.value.keywords and self.m.alone(node):
            # `m = import_helper.import_fresh_module("test.typinganndata.ann_module3")` in a test: a generated script
            # is a fresh process, so its first import of the module is fresh
            self.m.edit(*self.m.span(node), f"import {fresh[1]} as {fresh[0]}")

    def visit_Expr(self, node):
        self.generic_visit(node)
        call = node.value
        if isinstance(call, ast.Tuple) and len(call.elts) == 1:
            call = call.elts[0]  # `self.assertEqual(...),` with a stray comma is a one-tuple statement
        if not isinstance(call, ast.Call):
            return
        f = call.func
        if isinstance(f, ast.Name) and f.id in self.alias:
            name = self.alias[f.id].value.attr
            lines = rewrite_assertion(self.m, node, name, call)
            if lines:
                sep = "\n" + " " * node.col_offset if self.m.alone(node) else "; "
                if "\n" in sep or not any(ln.startswith((" ", "try:", "except", "else:")) for ln in lines):
                    self.m.edit(*self.m.span(node), sep.join(lines))
            return
        if isinstance(f, ast.Attribute) and f.attr == "__init__" and "TestCase" in ast.dump(f.value):
            self.m.edit(*self.m.span(node), "pass")
        elif (isinstance(f, ast.Attribute) and isinstance(f.value, ast.Name) and f.value.id in RECEIVERS and f.attr == "skipTest"
              and len(call.args) <= 1 and not call.keywords):
            self.m.edit(*self.m.span(node), "raise _conf_Skip(" + "".join(self.m.expr(a) for a in call.args) + ")")
        elif (isinstance(f, ast.Attribute) and isinstance(f.value, ast.Name) and f.value.id in RECEIVERS and f.attr == "addCleanup"
              and call.args and not call.keywords and not any(isinstance(a, ast.Starred) for a in call.args)):
            # unittest calls each cleanup after tearDown, last added first; the arguments are taken now
            fn, *rest = [self.m.expr(a) for a in call.args]
            self.m.edit(*self.m.span(node), f"_conf_cleanups.append(({fn}, ({''.join(r + ', ' for r in rest)})))")
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
            self.visit_multi_with(node)
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
        elif c.func.attr in ("assertWarns", "assertWarnsRegex") and len(c.args) == (2 if c.func.attr == "assertWarnsRegex" else 1) and not c.keywords:
            # the same block, recorded: _conf_Warns prints whether a matching warning was raised
            self.m.edit(*self.m.span(c), "_conf_Warns(" + ", ".join(self.m.expr(a) for a in c.args) + ")")
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

    def visit_multi_with(self, node):
        """`with self.subTest(x), self.assertRaises(E):` and the like: subTest is dropped (a failure stops the
        extracted test either way), and each assertRaises or assertWarns becomes the matching context manager."""
        def receiver_call(item):
            c = item.context_expr
            return (c.func.attr if isinstance(c, ast.Call) and isinstance(c.func, ast.Attribute) and isinstance(c.func.value, ast.Name)
                    and c.func.value.id in RECEIVERS else None)
        kinds = [receiver_call(i) for i in node.items]
        if not any(kinds):
            return
        new = []
        for item, k in zip(node.items, kinds):
            c = item.context_expr
            if k == "subTest":
                continue
            if k in ("assertRaises", "assertRaisesRegex") and item.optional_vars is None and not c.keywords and len(c.args) == (2 if k == "assertRaisesRegex" else 1):
                new.append("_conf_Raises(" + ", ".join(self.m.expr(a) for a in c.args) + ")")
            elif k in ("assertWarns", "assertWarnsRegex") and item.optional_vars is None and not c.keywords and len(c.args) == (2 if k == "assertWarnsRegex" else 1):
                new.append("_conf_Warns(" + ", ".join(self.m.expr(a) for a in c.args) + ")")
            elif k is None:
                new.append(self.m.expr(c) + (" as " + self.m.expr(item.optional_vars) if item.optional_vars is not None else ""))
            else:
                return  # not understood: left for the final check
        first, last = node.items[0], node.items[-1]
        end = last.optional_vars or last.context_expr
        start = self.m.off(first.context_expr.lineno, first.context_expr.col_offset)
        stop = self.m.off(end.end_lineno, end.end_col_offset)
        if new:
            self.m.edit(start, stop, ", ".join(new))

    def fix_uses(self, node, name):
        """`cm.exception` read inside the with body itself, before the name was known."""
        for sub in ast.walk(node):
            if isinstance(sub, ast.Attribute) and sub.attr == "exception" and isinstance(sub.value, ast.Name) and sub.value.id == name:
                self.m.edit(*self.m.span(sub), name + "_exception")


def caught_lines(exc, with_message):
    shown = "type(_e).__name__, str(_e)" if with_message else "type(_e).__name__"
    return [f"except {exc} as _e:", f"    print('raises', {shown})", "else:", f"    print('{NO_RAISE}')"]


def is_module_name(m, node):
    """Whether node is a name the test file binds with a plain `import`."""
    return isinstance(node, ast.Name) and m.imports.get(node.id, ("",))[0] == "import"


def rewrite_assertion(m, stmt, name, call):
    """The lines that replace `self.<name>(...)`, or None to leave it for the final check."""
    if any(k.arg not in ("msg", None) for k in call.keywords) and not name.startswith("assertRaises"):
        return None
    args = call.args
    if any(isinstance(a, ast.Starred) for a in args[:2]):
        return None
    t = [m.expr(a) for a in args]
    if name in SAME_AS_EQUAL and len(t) >= 2 and any(is_module_name(m, a) for a in args[:2]):
        # a module's repr names the file it was loaded from, which differs between machines: compare instead
        return [f"print(({t[0]}) == ({t[1]}))"]
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
    if name in ("assertRegex", "assertNotRegex") and len(t) == 2:
        return [f"_conf_search({t[0]}, {t[1]})"]
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
    return (loaded - stored) | global_reads(text), attrs


def global_reads(text):
    """Names some scope in the code reads as a global, even though another scope binds the same name locally
    (`prod` called in the test, and the loop variable `prod` in a nested function): the flat sets cannot tell."""
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            top = symtable.symtable("class _:\n" + text if text[:1].isspace() else text, "<test>", "exec")
    except SyntaxError:
        return set()
    out, todo = set(), [top]
    while todo:
        t = todo.pop()
        todo += t.get_children()
        out.update(s.get_name() for s in t.get_symbols() if s.is_referenced() and s.is_global() and not s.is_assigned())
    return out


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
        self.python = None  # the reference interpreter, set before tests are cut out: skip conditions are evaluated by it
        self.companions, self.last_files = {}, {}  # COMPANIONS' contents; the companion files of the last extracted test

    def chain(self, mod, cls):
        """The class and its bases, nearest first, ending before unittest.TestCase; None if not a test."""
        out = [(mod, cls)]
        bases = [b for b in cls.bases if not (isinstance(b, ast.Name) and b.id == "object")]
        if not bases:
            return None
        if len(bases) > 1:
            # `class TestPartialC(TestPartial, unittest.TestCase)`: mixins, then TestCase last. The MRO is then the
            # class, each mixin's own line of bases in order, and TestCase, when no class appears twice.
            if not self.is_testcase(bases[-1]):
                raise CantExtract("more than one base class")
            for b in bases[:-1]:
                line = self.mixin_line(mod, b)
                if line is None or any(c in out for c in line):
                    raise CantExtract("more than one base class")
                out += line
            return out
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

    @staticmethod
    def is_testcase(b):
        return isinstance(b, ast.Attribute) and isinstance(b.value, ast.Name) and b.attr == "TestCase" and b.value.id == "unittest"

    def mixin_line(self, mod, b):
        """A mixin base and its own bases, nearest first, down to object: [(mod, class), ...], or None."""
        if not isinstance(b, ast.Name):
            return None
        found = [n for n in mod.defs.get(b.id, []) if isinstance(n, ast.ClassDef)]
        if len(found) != 1:
            return None
        c = found[0]
        bases = [x for x in c.bases if not (isinstance(x, ast.Name) and x.id == "object")]
        if c.keywords or c.decorator_list or len(bases) > 1:
            return None
        if not bases:
            return [(mod, c)]
        rest = self.mixin_line(mod, bases[0])
        return [(mod, c)] + rest if rest else None

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
        if (name, method) in EXCLUDED_TESTS:
            raise Excluded(EXCLUDED_TESTS[(name, method)])
        chain = self.chain(mod, cls)
        # `class TestLRUPy(TestLRU, unittest.TestCase): module = py_functools`: the same tests against the pure-Python
        # copy. Brevi has one copy of the module, which its C-copy twin already scores.
        class_level = [st for _, c in chain for st in c.body if not isinstance(st, (ast.FunctionDef, ast.AsyncFunctionDef))]
        if mod.py_copies and any(isinstance(n, ast.Name) and n.id in mod.py_copies for st in class_level for n in ast.walk(st)):
            raise Excluded("the pure-Python copy of a module CPython also tests in C (Brevi has one copy)")
        owner, fn = next((m, n) for m, c in chain for n in c.body
                         if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)) and n.name == method)
        for m, c in chain:
            if decorator_plan(self.python, m, c):
                raise CantExtract("decorator that changes how the class runs")
        wrappers = decorator_plan(self.python, owner, fn)
        raw = owner.data[owner.starts[fn.lineno - 1]:owner.off(fn.end_lineno, fn.end_col_offset)].decode()
        for pattern, why in EXCLUDED_SOURCE:
            if re.search(pattern, raw):
                raise Excluded(why)
        if isinstance(fn, ast.AsyncFunctionDef):
            raise CantExtract("async test")
        # unittest calls a test with no arguments, so any beyond self must have a default (`def test_x(self, step=1)`)
        if (fn.args.args[:1] == [] or fn.args.args[0].arg != "self" or len(fn.args.args) - 1 > len(fn.args.defaults)
                or fn.args.posonlyargs or fn.args.vararg or fn.args.kwarg or None in fn.args.kw_defaults):
            raise CantExtract("test method takes extra arguments")

        script = Script(self)
        script.test_fn = fn
        body = owner.bare(fn)
        names, attrs = uses(body)
        # exec("... self.assertEqual(...) ...") keeps its assertions in a string, out of the rewriter's reach: the class
        # gets a base with those methods (the build refuses exec either way; the reference run shows the script works)
        # (or hands them to it: `exec(code, {'fail': self.fail})`)
        script.exec_self = any(isinstance(n, ast.Call) and isinstance(n.func, ast.Name) and n.func.id in ("exec", "eval", "compile")
                               and any(isinstance(a, ast.Constant) and isinstance(a.value, str) and "self." in a.value
                                       or isinstance(a, ast.Attribute) and isinstance(a.value, ast.Name) and a.value.id == "self"
                                       and a.attr in EXEC_SHIM for arg in n.args for a in ast.walk(arg))
                               for n in ast.walk(fn))
        plain = (not script.exec_self and not self_used(body) and "super" not in names and not has_mangled(body)
                 and not any(n.name in ("setUp", "tearDown") for _, c in chain for n in c.body if isinstance(n, ast.FunctionDef)))
        if plain:
            text = dedent(body, fn.col_offset)
            text = re.sub(r"^def (\w+)\(self(?:, |\))", lambda mo: f"def {mo.group(1)}(" + ("" if mo.group(0).endswith(", ") else ")"), text, count=1)
            for n in names:
                script.need(owner, n)
            script.call = wrapped_call(wrappers, method)
            tail = f"{text}\n\n{script.call}\n"
        else:
            tail = script.class_shape(chain, cls_name, method, wrappers)
        header = f"# Extracted by tools/conformance.py from CPython {TAG} Lib/test/{name}.py, {cls_name}.{method}\n"
        header += "# (CPython's test suite is under the PSF licence: https://docs.python.org/3/license.html)\n"
        out = script.finish(header, tail)
        self.last_files = {COMPANIONS[m][0]: self.companions[m] for m in sorted(script.companions)}
        return out


SKIPS = {"skipIf": False, "skipUnless": True}  # unittest decorator -> the value of the condition that lets the test run


def condition_value(py, mod, test, trusted=False, cache={}):
    """What a skipIf/skipUnless condition is under the reference interpreter, or CantExtract if it reads anything
    we do not model. The expression is checked against a whitelist before it is evaluated, in a child process."""
    allowed = (ast.BoolOp, ast.UnaryOp, ast.Not, ast.Or, ast.And, ast.Compare, ast.Constant, ast.Name, ast.Load, ast.Attribute,
               ast.Call, ast.GtE, ast.Gt, ast.Lt, ast.LtE, ast.Eq, ast.NotEq, ast.Is, ast.IsNot, ast.In, ast.NotIn, ast.Tuple)
    text = ast.unparse(test)
    if isinstance(test, ast.Name) and test.id in getattr(mod, "fresh", ()):
        # `@skipUnless(c_functools, ...)`: the copy exists in the reference CPython, which has every C accelerator
        # module (the extracted script imports the module itself under that name)
        return True
    for n in ([] if trusted else ast.walk(test)):
        bad = not isinstance(n, allowed)
        if isinstance(n, ast.Name) and n.id not in CONDITION_NAMES:
            bad = True
        elif isinstance(n, ast.Name) and n.id in CONDITION_BINDINGS and mod.imports.get(n.id) != CONDITION_BINDINGS[n.id]:
            bad = True  # the name must be bound the way the prelude binds it
        elif isinstance(n, ast.Name) and n.id == "multiprocessing" and mod.name != "test.test_re":
            bad = True  # test_re binds it with a try/except import, which the prelude copies
        if isinstance(n, ast.Call) and not (isinstance(n.func, ast.Name) and n.func.id == "hasattr"
                                            or isinstance(n.func, ast.Attribute) and n.func.attr == "startswith"):
            bad = True
        if bad:
            raise CantExtract(f"skip condition `{text}` cannot be decided")
    if py is None:
        raise CantExtract(f"skip condition `{text}` cannot be decided without the reference interpreter")
    if (py, text) not in cache:
        r = subprocess.run([py, "-I", "-B", "-c", CONDITION_PRELUDE + f"print(bool({text}))"], capture_output=True, text=True, timeout=30)
        if r.returncode != 0 or r.stdout.strip() not in ("True", "False"):
            raise CantExtract(f"skip condition `{text}` cannot be evaluated")
        cache[(py, text)] = r.stdout.strip() == "True"
    return cache[(py, text)]


def decorator_plan(py, mod, node):
    """What CPython does with the decorators of a test (or test class): Excluded if one skips it or marks it as
    not a language test, CantExtract if one is not understood, else the wrappers the script must apply, outermost first.
    Each is ("pickle_deprecated",) or ("ignore_warnings", category name)."""
    wrappers, excluded, unknown = [], None, None
    for d in node.decorator_list:
        target = d.func if isinstance(d, ast.Call) else d
        dotted = ast.unparse(target)
        last = target.attr if isinstance(target, ast.Attribute) else getattr(target, "id", "")
        args = d.args if isinstance(d, ast.Call) else []
        kws = {k.arg: k.value for k in d.keywords} if isinstance(d, ast.Call) else {}
        try:
            if last in EXCLUDED_DECORATORS:
                excluded = excluded or EXCLUDED_DECORATORS[last]
            elif (dotted == "unittest.skipIf" and len(args) == 2 and not kws and ast.unparse(args[0]) == "support.MISSING_C_DOCSTRINGS"
                  and mod.imports.get("support") == ("from", "test", "support")):
                if condition_value(py, mod, ast.parse(MISSING_DOCSTRINGS_CONDITION, mode="eval").body, trusted=True):
                    excluded = excluded or "skipped by CPython: " + ast.literal_eval(args[1])
            elif dotted in ("unittest.skipIf", "unittest.skipUnless") and args and not kws:
                if condition_value(py, mod, args[0]) != SKIPS[last]:
                    why = ast.literal_eval(args[1]) if len(args) > 1 and isinstance(args[1], ast.Constant) else ast.unparse(args[0])
                    excluded = excluded or f"skipped by CPython: {why}"
            elif dotted == "support.requires_docstrings" and mod.imports.get("support") == ("from", "test", "support"):
                if not condition_value(py, mod, ast.parse(DOCSTRINGS_CONDITION, mode="eval").body, trusted=True):
                    excluded = excluded or "skipped by CPython: test requires docstrings"
            elif (dotted == "threading_helper.requires_working_threading" and isinstance(d, ast.Call) and not args and not kws
                  and mod.imports.get("threading_helper") == ("from", "test.support", "threading_helper")):
                if not condition_value(py, mod, ast.parse(THREADS_CONDITION, mode="eval").body, trusted=True):
                    excluded = excluded or "skipped by CPython: requires threading support"
            elif dotted == "support.skip_on_s390x" and not isinstance(d, ast.Call) and mod.imports.get("support") == ("from", "test", "support"):
                if condition_value(py, mod, ast.parse(S390X_CONDITION, mode="eval").body, trusted=True):
                    excluded = excluded or "skipped by CPython: skipped on s390x"
            elif dotted == "support.skip_if_pgo_task" and mod.imports.get("support") == ("from", "test", "support"):
                pass  # support.PGO is False outside the PGO build task, so this is the identity
            elif dotted == "support.impl_detail" and not kws and mod.imports.get("support") == ("from", "test", "support"):
                excluded = excluded or "tests CPython internals"  # runs on CPython, but says it is specific to it
            elif dotted == "pickle_deprecated" and mod.name == "test.test_itertools" and not args:
                wrappers.append(("pickle_deprecated",))
            elif (dotted == "warnings_helper.ignore_warnings" and not args and list(kws) == ["category"]
                  and isinstance(kws["category"], ast.Name) and kws["category"].id in ("DeprecationWarning", "PendingDeprecationWarning", "UserWarning", "ResourceWarning", "RuntimeWarning", "SyntaxWarning")):
                wrappers.append(("ignore_warnings", kws["category"].id))
            else:
                raise CantExtract(f"decorator @{dotted}")
        except CantExtract as e:
            unknown = unknown or e
    if excluded:
        raise Excluded(excluded)
    if unknown:
        raise unknown
    return wrappers


def wrapped_call(wrappers, target):
    """The expression that runs `target` the way its decorators make CPython run it."""
    for w in reversed(wrappers):
        target = f"_conf_pickle_deprecated({target})" if w[0] == "pickle_deprecated" else f"_conf_ignore_warnings({w[1]})({target})"
    return target + "()"


def self_used(text):
    """Whether the method reads its own `self`; a nested method's `self` is another object."""
    def walk(node):
        for c in ast.iter_child_nodes(node):
            if isinstance(c, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda)) and any(a.arg == "self" for a in c.args.args):
                # its body's self is its own, but decorators and defaults run here: `@self.module.lru_cache(2)`
                outer = getattr(c, "decorator_list", []) + c.args.defaults + [d for d in c.args.kw_defaults if d]
                if any(walk(ast.Expression(o)) for o in outer):
                    return True
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


class FreshMember:
    """A class member inside `if c_functools:`, one indent deeper than the class body."""

    def __init__(self, node):
        self.node, self.lineno = node, node.lineno


def flat_body(mod, body):
    """A class body with each `if <fresh module>:` block opened up, so every member in it can be picked on its own."""
    for node in body:
        if isinstance(node, ast.If) and isinstance(node.test, ast.Name) and node.test.id in mod.fresh and not node.orelse:
            yield from (FreshMember(sub) for sub in node.body)
        else:
            yield node


@functools.lru_cache(None)
def reference_has(py, module):
    """Whether the reference CPython can import a module (its C test modules are optional in a build)."""
    if py is None:
        raise CantExtract(f"whether CPython has {module} cannot be decided without the reference interpreter")
    return subprocess.run([py, "-I", "-c", f"import {module}"], capture_output=True, timeout=30).returncode == 0


def support_special(py, name):
    """(text, imports) standing in for a test.support helper whose own code reaches CPython internals, or None for the
    helpers copied as they are. Raises Excluded where CPython itself skips the test or measures its C internals."""
    if name == "get_c_recursion_limit":
        # support: `import _testcapi; return _testcapi.Py_C_RECURSION_LIMIT`, else raise unittest.SkipTest
        if reference_has(py, "_testcapi"):
            raise CantExtract("get_c_recursion_limit() reads _testcapi")
        raise Excluded("skipped by CPython: get_c_recursion_limit() requires _testcapi, which the reference build lacks")
    if name in ("infinite_recursion", "get_recursion_depth"):
        if reference_has(py, "_testinternalcapi"):
            raise Excluded("measures the recursion depth with _testinternalcapi (CPython's C internals)")
        raise CantExtract(f"{name}() without _testinternalcapi")
    if name == "setswitchinterval":
        # support's version only raises the interval on an old Android emulator
        return "def setswitchinterval(interval):\n    return sys.setswitchinterval(interval)", ["import sys"]
    return None


class Script:
    """The definitions one test needs, gathered by following names from the test outward."""

    def __init__(self, universe):
        self.u, self.seen, self.nodes, self.lines, self.test_fn = universe, set(), {}, set(), None
        self.exec_self, self.call, self.companions = False, None, set()

    def need(self, mod, name):
        if (mod.name, name) in self.seen:
            return
        self.seen.add((mod.name, name))
        special = support_special(self.u.python, name) if mod.name == "test.support" else None
        if special:
            self.nodes[(-1, len(self.nodes))] = special[0]
            self.lines.update(special[1])
            return
        kind = mod.imports.get(name)
        companion = kind and (kind[1] if kind[0] == "import" else f"{kind[1]}.{kind[2]}")
        if companion in COMPANIONS and name not in mod.defs:
            self.lines.add(f"import {kind[1]}" + (f" as {kind[2]}" if kind[2] else "") if kind[0] == "import"
                           else f"from {kind[1]} import {kind[2]}" + (f" as {name}" if kind[2] != name else ""))
            return
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
                if star in ALLOWED_IMPORTS and name in star_names(star):
                    self.lines.add(f"from {star} import {name}")
                    break

    def include(self, mod, node):
        key = (self.u.order.index(mod.name), node.lineno)
        if key in self.nodes:
            return
        text = mod.block(node)
        self.nodes[key] = text
        for n in uses(text)[0]:
            self.need(mod, n)

    def class_shape(self, chain, leaf, method, wrappers):
        """A test that needs self keeps its class, with only the members it reaches, and its bases."""
        picked, todo = {}, [method, "__init__", "setUp", "tearDown", "setUpClass", "tearDownClass"]
        done = set()
        while todo:
            name = todo.pop()
            if name in done:
                continue
            done.add(name)
            for i, (mod, c) in enumerate(chain):
                for node in flat_body(mod, c.body):
                    text = self.member(mod, node, name)
                    if text is None:
                        continue
                    picked.setdefault(i, {})[node.lineno] = text
                    names, attrs = uses(text)
                    for n in names:
                        self.need(mod, n)
                    todo += sorted(attrs | names)
        out, base = [], ""
        if self.exec_self:
            base = "(_conf_TestCase)"
        elif any("super()" in t for ms in picked.values() for t in ms.values()):
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
        self.call = wrapped_call(wrappers, f"_t.{method}")
        calls.append(self.call)
        calls += ["_t.tearDown()"] if has("tearDown") else []
        calls += [f"{leaf}.tearDownClass()"] if has("tearDownClass") else []
        return "\n".join(out) + "\n" + "\n".join(calls) + "\n"

    def member(self, mod, node, name):
        if isinstance(node, FreshMember):
            # `if c_functools: module = c_functools`: the copy always exists (condition_value), so the body runs
            text = self.member(mod, node.node, name)
            return None if text is None else "\n".join(ln[4:] if ln.startswith("    ") else ln for ln in text.split("\n"))
        if node is self.test_fn:
            return mod.bare(node)  # its decorators were dealt with (decorator_plan)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name == name and node.name.startswith("test") and node.decorator_list:
            # another test the test calls (`self.test_count_threading(step=5)`): skip decorators that let it run under
            # the reference CPython are the identity there; one that would skip it skips the caller too (Excluded)
            if not decorator_plan(self.u.python, mod, node):
                return mod.bare(node)
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
        text = "\n".join(list(self.nodes.values()) + [tail])
        if re.search(r"\b_conf_(Skip|cleanups)\b", text):
            # skipTest ends the test where it is called; the cleanups run after it (and after tearDown), last first
            head, sep, rest = tail.rpartition(self.call)
            tail = head + f"_conf_run(lambda: {self.call})" + rest
            if "_conf_cleanups" in text:
                tail += "for _f, _a in reversed(_conf_cleanups):\n    _f(*_a)\n"
            text = "\n".join(list(self.nodes.values()) + [tail])
        used = [h for h in HELPERS if re.search(rf"\b{h}\b", text)]
        used += [d for h in used for d in HELPER_NEEDS.get(h, []) if d not in used]
        self.lines.update(i for h in used for i in HELPER_IMPORTS[h])
        future = sorted(ln for ln in self.lines if ln.startswith("from __future__"))
        imports = sorted(self.lines - set(future))
        parts = ([header + "\n".join(future + imports)] + [HELPERS[h] for h in HELPERS if h in used]
                 + [self.nodes[k] for k in sorted(self.nodes)] + [tail])
        script = "\n\n".join(p for p in parts if p.strip())
        validate(script)
        for n in ast.walk(parse(script)):
            # each companion module the script imports, and each package above it, travels with the script
            mods = ([a.name for a in n.names] if isinstance(n, ast.Import)
                    else [f"{n.module}.{a.name}" for a in n.names] + [n.module] if isinstance(n, ast.ImportFrom) else [])
            for m in mods:
                parts = m.split(".")
                if m in COMPANIONS:
                    self.companions.update(".".join(parts[:i]) for i in range(1, len(parts) + 1))
        return script


def validate(script):
    """The last gate: only what the rewriting fully handled, and only safe imports, may run."""
    try:
        tree = parse(script)
    except SyntaxError as e:
        raise CantExtract(f"generated script does not parse: {e.msg}")
    defined = {n.name for n in ast.walk(tree) if isinstance(n, ast.FunctionDef)}
    # the attribute reads of a restricted module that are allowed: `io.StringIO`, `unittest.mock`
    allowed_reads = {id(n.value) for n in ast.walk(tree) if isinstance(n, ast.Attribute) and isinstance(n.value, ast.Name)
                     and n.attr in RESTRICTED_IMPORTS.get(n.value.id, ())}
    for n in ast.walk(tree):
        roots = []
        if isinstance(n, ast.Import):
            for a in n.names:
                root, _, rest = a.name.partition(".")
                if a.name in COMPANIONS:
                    continue
                if root in RESTRICTED_IMPORTS and not a.asname and (not rest or rest in RESTRICTED_IMPORTS[root]):
                    continue  # `import io`, `import unittest.mock`: every read of the name is checked below
                roots.append(root if root not in RESTRICTED_IMPORTS else a.name)
        elif isinstance(n, ast.ImportFrom):
            root, _, rest = (n.module or "").partition(".")
            if n.level == 0 and n.module in COMPANIONS and all(f"{n.module}.{a.name}" in COMPANIONS for a in n.names):
                pass
            elif root in RESTRICTED_IMPORTS and n.level == 0:
                if not (rest in RESTRICTED_IMPORTS[root] or not rest and all(a.name in RESTRICTED_IMPORTS[root] for a in n.names)):
                    roots.append(n.module)
            else:
                roots.append(root)
        for r in roots:
            if r not in ALLOWED_IMPORTS:
                raise CantExtract(f"imports {r}")
        if isinstance(n, ast.Name) and n.id in RESTRICTED_IMPORTS and id(n) not in allowed_reads and isinstance(n.ctx, ast.Load):
            raise CantExtract(f"uses {n.id} beyond {', '.join(sorted(RESTRICTED_IMPORTS[n.id]))}")
        if isinstance(n, ast.Name) and n.id in FORBIDDEN_NAMES and isinstance(n.ctx, ast.Load):
            raise CantExtract(f"uses {n.id}()")
        if (isinstance(n, ast.Attribute) and isinstance(n.value, ast.Name) and n.value.id in RECEIVERS
                and n.attr.startswith("assert") and n.attr not in defined):  # a helper the test class defines is fine
            raise CantExtract(f"unittest API {n.attr}")
        f = n.func if isinstance(n, ast.Call) else None
        if isinstance(f, ast.Attribute) and isinstance(f.value, ast.Name) and f.value.id in RECEIVERS and f.attr in TEST_API:
            raise CantExtract(f"unittest API {f.attr}")


def cache_dir():
    base = os.environ.get("BREVI_CACHE")
    return (Path(base) if base else Path.home() / ".cache" / "brevi").resolve() / "conformance"


def load_sources(cache, pinned=None):
    """Every pinned file, from the cache if it is there, else from GitHub; each must match its hash."""
    out = {}
    for name, (path, sha) in (SOURCES if pinned is None else pinned).items():
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


WINDOWS = os.name == "nt"


def run_limited(cmd, cwd, env, timeout, mem_mb, sandbox):
    """Runs a generated script or built program in its own process group: no network and writes only to
    its folder where sandbox-exec exists, CPU, output-size and (on Linux) address-space limits through
    ulimit, and on every platform a wall-clock timeout and a resident-memory watch."""
    if WINDOWS:
        return run_limited_windows(cmd, cwd, env, timeout, mem_mb)
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


def run_limited_windows(cmd, cwd, env, timeout, mem_mb):
    """Windows has no ulimit or killpg: a wall-clock timeout and a resident-memory watch, and the whole process
    tree is killed with taskkill when either trips."""
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        p = subprocess.Popen(cmd, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                             creationflags=subprocess.CREATE_NEW_PROCESS_GROUP)
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
                    subprocess.run(["taskkill", "/F", "/T", "/PID", str(p.pid)], capture_output=True)
                    p.wait()
        out.seek(0)
        err.seek(0)
        return Run(p.returncode, out.read(), err.read(), note)


# NTSTATUS codes a crashed Windows program exits with, as the POSIX signal of the same kind (so the private
# handling of possible memory-unsafety in is_private covers Windows too).
NT_CRASH = {0xC0000005: "SIGSEGV", 0xC00000FD: "SIGSEGV", 0xC0000409: "SIGABRT", 0xC000001D: "SIGILL",
            0xC0000094: "SIGFPE", 0x80000003: "SIGTRAP", 0xC0000374: "SIGABRT"}


def resident_mb(pid):
    if WINDOWS:
        import ctypes
        from ctypes import wintypes

        class Counters(ctypes.Structure):
            _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
                (n, ctypes.c_size_t) for n in ("PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage",
                                               "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage",
                                               "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage")]
        k32 = ctypes.WinDLL("kernel32")
        h = k32.OpenProcess(0x1000 | 0x0010, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ
        if not h:
            return 0
        c = Counters(cb=ctypes.sizeof(Counters))
        ok = k32.K32GetProcessMemoryInfo(h, ctypes.byref(c), c.cb)
        k32.CloseHandle(h)
        return c.WorkingSetSize // (1024 * 1024) if ok else 0
    r = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)], capture_output=True, text=True)
    return int(r.stdout.strip() or 0) // 1024


def sandbox_works():
    if not shutil.which("sandbox-exec"):
        return False
    r = subprocess.run(["sandbox-exec", "-p", "(version 1)(allow default)(deny network*)", "/usr/bin/true"], capture_output=True)
    return r.returncode == 0


def run_env(folder, seed):
    if WINDOWS:
        keep = {k: os.environ[k] for k in ("SystemRoot", "SystemDrive", "PATH", "PATHEXT", "COMSPEC", "WINDIR") if k in os.environ}
        return {**keep, "USERPROFILE": str(folder), "TEMP": str(folder), "TMP": str(folder), "PYTHONUTF8": "1",
                "PYTHONHASHSEED": str(seed), "PYTHONDONTWRITEBYTECODE": "1"}
    return {"PATH": "/usr/bin:/bin", "HOME": str(folder), "TMPDIR": str(folder), "PYTHONUTF8": "1",
            "PYTHONHASHSEED": str(seed), "PYTHONDONTWRITEBYTECODE": "1"}


ADDRESS = re.compile(rb" at 0x[0-9a-fA-F]+>")


def masked(run):
    """The run with object addresses in default reprs (`<function f at 0x7f...>`) blanked. An address is where the
    object happens to live in this process, so it differs on every CPython run and can never match a build's."""
    return run._replace(out=ADDRESS.sub(b" at 0x?>", run.out)) if run else run


def reference(py, script, folder, seed, cfg):
    # -P keeps the script's folder off sys.path, unless the test's companion modules (COMPANIONS) are there to import
    safe = [] if (folder / "test").is_dir() else ["-P"]
    return masked(run_limited([py, "-B", "-s", *safe, str(script)], str(folder), run_env(folder, seed), cfg.timeout, cfg.mem_mb, cfg.sandbox))


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
        if WINDOWS and not exe.exists() and exe.with_suffix(".exe").exists():
            exe = exe.with_suffix(".exe")
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
    if WINDOWS and got.rc >= 0xC0000000 and not got.note:
        sig = NT_CRASH.get(got.rc)
        return "crash_or_timeout", f"signal {sig} (0x{got.rc:08X})" if sig else f"exception 0x{got.rc:08X}"
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


REFERENCE_TRIES = 2  # a reference run that timed out, hit the memory limit or died is tried once more
SETTLE_RUNS = 3  # normal runs at one seed before its output counts as not reproducible


class ReferenceFailed(Exception):
    def __init__(self, seed, why, tries):
        super().__init__(f"CPython reference run failed at PYTHONHASHSEED={seed}: {why} ({tries} tries)")
        self.seed, self.why, self.tries = seed, why, tries


def abnormal(run):
    """Why a CPython reference run tells nothing about the test (it timed out, hit the memory limit or died on a
    signal), or "". A run like that is an infrastructure error, never evidence that the output depends on the seed."""
    if run.note:
        return run.note
    if WINDOWS and run.rc >= 0xC0000000:
        return f"exception 0x{run.rc:08X}"
    if run.rc < 0:
        try:
            return f"signal {signal.Signals(-run.rc).name}"
        except ValueError:
            return f"signal {-run.rc}"
    return ""


def outcome(run):
    return run.rc, run.out


class Reference:
    """CPython's reference runs of one case, kept per PYTHONHASHSEED so both builds share them. A single run can be
    disturbed (a timeout under load, a killed process), so no single run decides what is scored, in either
    direction: an abnormal run is retried and then reported as an error (ReferenceFailed), and before a seed's output
    is called different from another seed's, it is settled: two normal runs at that seed must agree."""

    def __init__(self, run):
        self.run, self.runs = run, {}  # run(seed) -> Run; seed -> that seed's normal runs, in order

    def one(self, seed):
        """One more normal run at `seed`, or ReferenceFailed after REFERENCE_TRIES abnormal ones."""
        for _ in range(REFERENCE_TRIES):
            r = self.run(seed)
            why = abnormal(r)
            if not why:
                self.runs.setdefault(seed, []).append(r)
                return r
        raise ReferenceFailed(seed, why, REFERENCE_TRIES)

    def first(self, seed):
        return self.runs[seed][0] if self.runs.get(seed) else self.one(seed)

    def settled(self, seed):
        """The output two normal runs at `seed` agree on, or None when SETTLE_RUNS normal runs all differ (the output
        does not depend on the seed alone)."""
        while True:
            seen = {}
            for r in self.runs.get(seed, []):
                if outcome(r) in seen:
                    return seen[outcome(r)]
                seen[outcome(r)] = r
            if len(self.runs.get(seed, [])) >= SETTLE_RUNS:
                return None
            self.one(seed)


def unreproducible(seed):
    return f"rule 1: CPython's output is not reproducible: {SETTLE_RUNS} runs at PYTHONHASHSEED={seed} all differ"


def stable_reference(ref, random_output):
    """(CPython's settled Run for the case, None) or (None, the unstable detail). Rule 1: seeds 1 and 2 must agree;
    when their first runs differ, each seed is settled first, so one disturbed run cannot drop a test from the score."""
    a = ref.first(1)
    if random_output:
        return a, None  # the output is random by design: only the exit code and crashes are compared
    b = ref.first(2)
    if outcome(a) == outcome(b):
        return a, None
    for seed in (1, 2):
        if ref.settled(seed) is None:
            return None, unreproducible(seed)
    a, b = ref.settled(1), ref.settled(2)
    if outcome(a) != outcome(b):
        return None, "rule 1: CPython's output differs between PYTHONHASHSEED=1 and 2 (each seed reproduced)"
    return a, None


def seed_dependence(ref, want):
    """Rule 2, for a build whose output differs from CPython's: two seeds can agree by luck on a small str set, so
    seeds 3 to 5 are tried before calling it Brevi's fault. Returns (unstable detail or None, note for the wrong
    result or ""). A seed's run that matches `want` needs no second run; one that differs is settled first."""
    for seed in (3, 4, 5):
        try:
            if outcome(ref.first(seed)) == outcome(want):
                continue
            r = ref.settled(seed)
        except ReferenceFailed as e:
            return None, f" (seed check stopped: {e})"
        if r is None:
            return unreproducible(seed).replace("rule 1", "rule 2"), ""
        if outcome(r) != outcome(want):
            return f"rule 2: CPython's output at PYTHONHASHSEED={seed} differs from seeds 1 and 2 (reproduced)", ""
    return None, ""


def run_case(case, cfg):
    """Fills in the case's result for each mode."""
    folder = cfg.work / case["id"]
    shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True)
    script = folder / "case.py"
    script.write_text(case["script"], encoding="utf-8")
    for rel, data in case.get("files", {}).items():
        (folder / rel).parent.mkdir(parents=True, exist_ok=True)
        (folder / rel).write_bytes(data)
    ref = Reference(lambda seed: reference(cfg.python, script, folder, seed, cfg))
    random_output = bool(RANDOM_OUTPUT.match(case["id"]))
    try:
        a, unstable = stable_reference(ref, random_output)
    except ReferenceFailed as e:
        return {**case, "category": "not_extractable", "detail": str(e)}
    if unstable:
        return {**case, "category": "unstable", "detail": unstable}
    if a.rc != 0:
        return {**case, "category": "not_extractable", "detail": f"generated script fails under CPython: {last_line(a.err)}"}
    if NO_RAISE.encode() in a.out:
        return {**case, "category": "not_extractable", "detail": "assertRaises saw no exception in the extracted test"}
    if SKIPPED in a.out:
        # the test called skipTest under the reference CPython (a locale this machine lacks, say): CPython skips it
        why = a.out[a.out.index(SKIPPED) + len(SKIPPED):].split(b"\n")[0].decode("utf-8", "replace").strip()
        return {**case, "category": "excluded", "detail": f"skipped by CPython at run time: {why}"}
    modes = {}
    for mode in ("normal", "dyn"):
        exe, path, why, crashed = build(script, folder, mode, cfg)
        got = masked(run_limited([str(exe)], str(folder), run_env(folder, 1), cfg.timeout, cfg.mem_mb, cfg.sandbox)) if exe else None
        category, detail = classify(a, (exe is not None, why, crashed), got)
        if random_output and category in ("matched", "wrong_output"):
            category, detail = "unstable", "CPython's output depends on unseeded random numbers (checked for crashes and exit code only)"
        if category == "wrong_output":
            unstable, note = seed_dependence(ref, a)
            if unstable:
                return {**case, "category": "unstable", "detail": unstable}
            detail += note
        modes[mode] = {"category": category, "path": (path or {}).get("path"), "detail": detail, "silent": not a.out,
                       "stderr": last_line(got.err) if got else ""}
    shutil.rmtree(folder, ignore_errors=True)
    return {**case, "modes": modes}


CATEGORIES = ["matched", "refused", "by_design", "string_model", "cpython_private", "wrong_output", "wrong_exit", "crash_or_timeout", "not_extractable", "unstable", "excluded"]
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
    total = len(results) - counts["unstable"] - counts["excluded"] - counts["by_design"]
    native = counts["matched"] - bridged
    silent = sum(1 for r in results if "modes" in r and r["modes"][mode]["category"] == "matched" and r["modes"][mode]["silent"])
    native_rows = [r for r in results if "modes" in r and r["modes"][mode]["category"] == "matched" and r["modes"][mode]["path"] != "pybridge"]
    # Assertion-bearing: a native match whose program printed something to compare. A silent match only shows it
    # built and exited 0, so it is weaker evidence and is published apart.
    asserting = sum(1 for r in native_rows if not r["modes"][mode]["silent"])
    typed = sum(1 for r in native_rows if r["modes"][mode]["path"] == "typed")
    return {**counts, "total": len(results), "scored": total, "matched_through_cpython": bridged, "matched_silent": silent,
            "coverage": round(native / total, 4) if total else 0, "wrong": counts["wrong_output"] + counts["wrong_exit"],
            "matched_asserting": asserting, "assert_coverage": round(asserting / total, 4) if total else 0,
            "matched_typed": typed, "typed_share": round(typed / native, 4) if native else 0}


def read_by_design():
    """Test ids from the reviewed by-design list: `test-id  why`, one per line, `#` comments."""
    if not BY_DESIGN_FILE.exists():
        return set()
    return {ln.split()[0] for ln in BY_DESIGN_FILE.read_text().splitlines() if ln.strip() and not ln.startswith("#")}


def by_design_refusal(r):
    """Whether the dynamic build refused this test because it runs a string as code (and nothing built it)."""
    return ("modes" in r and r["modes"]["dyn"]["category"] in ("refused", "by_design")
            and bool(BY_DESIGN_REFUSAL.match(r["modes"]["dyn"]["detail"] or ""))
            and r["modes"]["normal"]["category"] in ("refused", "by_design"))


def read_string_model():
    """{test id: issue} from the reviewed needs-the-string-model list: `test-id  issue=#N`, `#` comments."""
    if not STRING_MODEL_FILE.exists():
        return {}
    rows = [ln.split() for ln in STRING_MODEL_FILE.read_text().splitlines() if ln.strip() and not ln.startswith("#")]
    return {r[0]: next((t[len("issue="):] for t in r[1:] if t.startswith("issue=#")), "") for r in rows}


def string_model_stop(r):
    """Whether both builds stopped on the lone-surrogate refusal and on nothing else."""
    return "modes" in r and all(x["category"] in ("wrong_exit", "string_model") and bool(STRING_MODEL_STOP.match(x["detail"] or ""))
                                for x in r["modes"].values())


def read_private():
    """{test id: issue} from the reviewed CPython-private-helper list: `test-id  issue=#N`, `#` comments."""
    if not PRIVATE_FILE.exists():
        return {}
    rows = [ln.split() for ln in PRIVATE_FILE.read_text().splitlines() if ln.strip() and not ln.startswith("#")]
    return {r[0]: next((t[len("issue="):] for t in r[1:] if t.startswith("issue=#")), "") for r in rows}


def private_refusal(r):
    """Whether the dynamic build refused exactly a CPython-private module attribute and the normal build refused too."""
    return ("modes" in r and r["modes"]["dyn"]["category"] in ("refused", "cpython_private")
            and bool(PRIVATE_REFUSAL.match(r["modes"]["dyn"]["detail"] or ""))
            and r["modes"]["normal"]["category"] in ("refused", "cpython_private"))


def bad_ids(results):
    return sorted(f"{r['id']} {mode}" for r in results if "modes" in r for mode in r["modes"] if r["modes"][mode]["category"] in BAD)


def read_floor():
    return json.loads(FLOOR.read_text()) if FLOOR.exists() else {}


META_KEYS = ("issue", "owner", "expires")


def entry_name(key):
    """`test-id mode` -> its file name in the known-wrong folder: `test-id.mode`."""
    tid, mode = key.split()
    return f"{tid}.{mode}"


def read_baseline():
    """`test-id mode` -> its metadata ({issue, owner, expires}); every key must be filled in (see check_meta).

    One file per entry, named `test-id.mode` and holding `issue=#N owner=LOGIN expires=YYYY-MM-DD`, so two PRs that
    add or remove different entries never touch the same file and never conflict (a single list conflicted whenever
    two PRs changed neighbouring lines)."""
    if not BASELINE.is_dir():
        return {}
    out = {}
    for f in sorted(BASELINE.iterdir()):
        if f.name == "README.md" or f.name.startswith("."):
            continue
        tid, _, mode = f.name.rpartition(".")
        out[f"{tid} {mode}"] = dict(w.split("=", 1) for w in f.read_text().split() if "=" in w)
    return out


def check_meta(known, today):
    """Every known-wrong line names a tracking issue, an owner and an expiry date, and is not past it."""
    bad = []
    for k, meta in sorted(known.items()):
        missing = [m for m in META_KEYS if not meta.get(m) or meta[m] == "?"]
        if missing:
            bad.append(f"{k}: missing {', '.join(missing)}")
        elif not re.fullmatch(r"#\d+", meta["issue"]) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", meta["expires"]):
            bad.append(f"{k}: issue must be #N and expires YYYY-MM-DD")
        elif meta["expires"] < today:
            bad.append(f"{k}: expired on {meta['expires']} (fix it, or renew the date on its issue with a reason)")
    return bad


def write_baseline(lines, known):
    """Makes the known-wrong folder hold exactly `lines`, keeping the metadata of entries already there; a new entry
    gets `?` values, which --check refuses until they are filled in."""
    BASELINE.mkdir(parents=True, exist_ok=True)
    want = {entry_name(k): k for k in lines}
    for f in BASELINE.iterdir():
        if f.name != "README.md" and not f.name.startswith(".") and f.name not in want:
            f.unlink()
    for name, k in want.items():
        m = known.get(k, {})
        text = " ".join(f"{key}={m.get(key) or '?'}" for key in META_KEYS) + "\n"
        if not (BASELINE / name).exists() or (BASELINE / name).read_text() != text:
            (BASELINE / name).write_text(text)


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
        out = ["| File | " + " | ".join(CATEGORIES) + " | total | coverage | wrong |",
               "| --- | " + " | ".join("---:" for _ in CATEGORIES) + " | ---: | ---: | ---: |"]
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
             f"**Assertion-bearing coverage {n['assert_coverage'] * 100:.1f}%** ({n['matched_asserting']} of {n['scored']}: native matches that printed "
             f"something to compare; {n['matched_silent']} matches print nothing). **Typed-path share {n['typed_share'] * 100:.1f}%** "
             f"({n['matched_typed']} of {n['matched'] - n['matched_through_cpython']} native matches took the typed build).", "",
             f"Forced dynamic build (`BREVI_DYN=1`): coverage {d['coverage'] * 100:.1f}%, wrong {d['wrong']}.", "",
             f"**Refused by design: {n['by_design']}** tests run a string as code (compile, exec or eval), which a sandboxed "
             "native binary refuses on purpose. They are left out of the coverage denominator and listed by name below.", "",
             f"**Need the string model: {n['string_model']}** tests stop on a str holding a lone surrogate, which Brevi's "
             "strings cannot hold yet. They count as not matched and are listed by name below.", "",
             f"**Call CPython-private helpers: {n['cpython_private']}** tests call a private module function (such as "
             "re._compile or functools._c3_mro) that Brevi refuses. They count as not matched and are listed by name below.", "",
             "## Normal build", ""] + table("normal") + ["", "## Forced dynamic build", ""] + table("dyn")
    lines += ["", "## Paths taken", "", "| Build | " + " | ".join(rep["paths"]["normal"]) + " |",
              "| --- | " + " | ".join("---:" for _ in rep["paths"]["normal"]) + " |",
              "| normal | " + " | ".join(str(v) for v in rep["paths"]["normal"].values()) + " |"]
    lines += ["", f"{n['matched_through_cpython']} matched programs on the normal build ran library calls in CPython (`pybridge`). "
              "They stay in `matched` but do not count towards coverage.",
              f"{n['matched_silent']} matched programs print nothing: the test has no assertion, so a match means it built and exited 0."]
    lines += ["", "## Refused by design (strings run as code)", "",
              f"Reviewed list: [compat/conformance_by_design.txt](../../compat/{BY_DESIGN_FILE.name}). `--check` fails on one not in it.", ""]
    lines += [f"- `{r['id']}`: {r['modes']['dyn']['detail']}" for r in results
              if "modes" in r and r["modes"]["normal"]["category"] == "by_design"]
    sm = read_string_model()
    lines += ["", "## Need the string model (lone surrogates)", "",
              f"Reviewed list: [compat/{STRING_MODEL_FILE.name}](../../compat/{STRING_MODEL_FILE.name}), each tied to an issue.", ""]
    lines += [f"- `{r['id']}` ({sm.get(r['id'], '')}): {r['modes']['normal']['detail']}" for r in results
              if "modes" in r and r["modes"]["normal"]["category"] == "string_model"]
    pv = read_private()
    lines += ["", "## Call CPython-private helpers", "",
              f"Reviewed list: [compat/{PRIVATE_FILE.name}](../../compat/{PRIVATE_FILE.name}), each tied to an issue.", ""]
    lines += [f"- `{r['id']}` ({pv.get(r['id'], '')}): {r['modes']['dyn']['detail']}" for r in results
              if "modes" in r and r["modes"]["normal"]["category"] == "cpython_private"]
    lines += ["", "## Excluded (CPython internals, and tests CPython itself skips)", ""]
    why = collections.Counter(r["detail"] for r in results if r.get("category") == "excluded")
    lines += [f"- {w}: {c}" for w, c in why.most_common()]
    for title, cat in (("Why tests are not extractable", "not_extractable"),):
        lines += ["", f"## {title}", ""]
        why = collections.Counter(normalise(r["detail"]) for r in results if r.get("category") == cat)
        lines += [f"- {c}: {w}" for w, c in why.most_common(25)]
    lines += ["", "## Unstable (left out of the score)", "",
              "Rule 1: CPython's output at PYTHONHASHSEED 1 and 2, each seed settled by a second run when they differ. "
              "Rule 2: seeds 3 to 5, tried when a build's output differs.", ""]
    lines += [f"- `{r['id']}`: {r['detail']}" for r in results if r.get("category") == "unstable"]
    random = sum(1 for r in results if any(x["category"] == "unstable" for x in r.get("modes", {}).values()))
    if random:
        lines.append(f"- {random} more print unseeded random numbers (checked for crashes and exit code only).")
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


CANDIDATES = ROOT / "tools" / "conformance_candidates.json"


def use_candidates(slice_name):
    """Pin the candidate files (and the extra pure-Python imports they need) for a measuring run; returns the slice's files.
    A candidate never replaces a pinned source, so a published slice measures the same files with or without it."""
    data = json.loads(CANDIDATES.read_text())
    for name, (path, sha) in data["sources"].items():
        if name in SOURCES and SOURCES[name] != (path, sha):
            sys.exit(f"{CANDIDATES.name}: {name} differs from its pin in SOURCES")
        SOURCES[name] = (path, sha)
    ALLOWED_IMPORTS.update(data["imports"])
    return data["slices"][slice_name]


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
                if universe.last_files:
                    case["files"] = universe.last_files
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
    for rel in case.get("files", {}):
        print(f"# needs {rel} from CPython {TAG} Lib/ next to case.py (in the cache under cpython/{TAG}/{rel})")
    print(f"# Save this as case.py, then:\n#   {cfg.python} case.py\n#   BREVI_PATH=1 {cfg.brevi} build case.py -o prog && ./prog\n"
          "#   BREVI_PATH=1 BREVI_DYN=1 ... the same, for the forced dynamic build")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--slice", choices=sorted(SLICES), default="core", help="which published slice: its files, floor and known-wrong list (default: core)")
    ap.add_argument("--files", help="comma-separated test files (default: the slice's)")
    ap.add_argument("--filter", help="only test ids matching this regular expression")
    ap.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 1), help="parallel tests (default: at most 4)")
    ap.add_argument("--timeout", type=float, default=20, help="seconds per program run (default: %(default)s)")
    ap.add_argument("--build-timeout", type=float, default=300, help="seconds per build (default: %(default)s)")
    ap.add_argument("--mem-mb", type=int, default=2048, help="memory limit per program run (default: %(default)s)")
    ap.add_argument("--python", help=f"the CPython {PYTHON_VERSION} to compare with (default: python3.13 on PATH)")
    ap.add_argument("--out", type=Path, help="folder for report.json and report.md (default: the cache)")
    ap.add_argument("--check", action="store_true", help="exit 1 on a wrong result not in the baseline")
    ap.add_argument("--raise-floor", action="store_true", help="with --check: when the ratchet holds, raise the slice's floor to this run's matches (main only)")
    ap.add_argument("--release-gate", action="store_true", help="exit 1 below the slice's release_gate (absolute coverage, zero wrong)")
    ap.add_argument("--write-baseline", action="store_true", help="replace the slice's known-wrong list with this run's results; the floor only goes down (main raises it)")
    ap.add_argument("--show", metavar="ID", help="print one test's generated script and stop")
    ap.add_argument("--candidates", action="store_true", help=f"measure the slice's candidate files in {CANDIDATES.name} "
                    "instead (needs --out; never scored: refuses --check, --raise-floor, --release-gate, --write-baseline)")
    cfg = ap.parse_args()
    global BASELINE, FLOOR, SCOPE
    SCOPE, BASELINE, FLOOR = SLICES[cfg.slice]
    if cfg.candidates:
        if cfg.check or cfg.raise_floor or cfg.release_gate or cfg.write_baseline:
            sys.exit("--candidates only measures: it never checks, gates, or writes a floor or a baseline")
        if not cfg.out:
            sys.exit("--candidates needs --out (a folder outside bench/conformance)")
        if (ROOT / "bench" / "conformance").resolve() in [cfg.out.resolve(), *cfg.out.resolve().parents]:
            sys.exit("--candidates never writes into bench/conformance (main publishes those reports)")
        SCOPE = use_candidates(cfg.slice)
    cfg.files = cfg.files or ",".join(SCOPE)
    names = cfg.files.split(",")
    unknown = [n for n in names if "test." + n not in SOURCES]
    if unknown:
        sys.exit(f"not pinned in SOURCES: {', '.join(unknown)}")
    cache = cache_dir()
    universe = Universe(load_sources(cache))
    universe.companions = load_sources(cache, COMPANIONS)
    cfg.python = find_python(cfg.python)
    cfg.brevi = str(Path(os.environ.get("BREVI", ROOT / "target" / "release" / ("brevi.exe" if WINDOWS else "brevi"))).resolve())
    universe.python = cfg.python
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
        r.pop("files", None)
    reviewed = read_by_design()
    for r in results:
        if r["id"] in reviewed and by_design_refusal(r):
            for x in r["modes"].values():
                if x["category"] == "refused":
                    x["category"] = "by_design"
    string_model = read_string_model()
    for r in results:
        if r["id"] in string_model and string_model_stop(r):
            for x in r["modes"].values():
                x["category"] = "string_model"
    private_list = read_private()
    for r in results:
        if r["id"] in private_list and private_refusal(r):
            for x in r["modes"].values():
                if x["category"] == "refused":
                    x["category"] = "cpython_private"
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
    print(f"coverage {n['coverage'] * 100:.1f}%, wrong {n['wrong']} ({n['wrong_output']} output, {n['wrong_exit']} exit), "
          f"{n['by_design']} refused by design (strings run as code, not counted), "
          f"{n['string_model']} need the string model, {n['cpython_private']} call CPython-private helpers "
          f"(both counted as not matched); report in {out}")
    bad = set(bad_ids(results))
    matched = {m: rep["totals"][m]["matched"] for m in ("normal", "dyn")}
    if cfg.write_baseline:
        write_baseline(bad, read_baseline())
        # Only ever lowers the floor (a regression accepted on purpose): main raises it after each merge (--raise-floor),
        # so a PR that edits it conflicts with every other open PR.
        old = read_floor().get("matched", {})
        low = {m: min(n, old.get(m, n)) for m, n in matched.items()}
        FLOOR.write_text(json.dumps({**read_floor(), "matched": low}, indent=1) + "\n")
    if cfg.check:
        # The ratchet: the score only goes up, and the list of known-wrong results only shrinks.
        known, floor, failed = read_baseline(), read_floor().get("matched", {}), False
        ran = {r["id"] for r in results}  # --filter and --files say nothing about the tests they leave out
        new, gone = sorted(bad - set(known)), sorted(k for k in set(known) - bad if k.split()[0] in ran)
        unowned = check_meta(known, time.strftime("%Y-%m-%d"))
        unreviewed = sorted(r["id"] for r in results if r["id"] not in reviewed and by_design_refusal(r))
        stale = sorted(i for i in reviewed if i in ran and not any(r["id"] == i and by_design_refusal(r) for r in results))
        if unreviewed:
            print(f"{len(unreviewed)} tests are refused for running a string as code but are not in the reviewed list; read each "
                  f"and add it to {BY_DESIGN_FILE.relative_to(ROOT)} (the list grows only by a reviewed edit):\n  " + "\n  ".join(unreviewed))
            failed = True
        if stale:
            print(f"{len(stale)} tests in {BY_DESIGN_FILE.relative_to(ROOT)} are no longer refused for running a string as code; "
                  "remove them:\n  " + "\n  ".join(stale))
            failed = True
        sm_stale = sorted(i for i in string_model if i in ran and not any(r["id"] == i and string_model_stop(r) for r in results))
        sm_untied = sorted(i for i, issue in string_model.items() if not issue)
        if sm_stale:
            print(f"{len(sm_stale)} tests in {STRING_MODEL_FILE.relative_to(ROOT)} no longer stop on the lone-surrogate refusal "
                  "in both builds; remove them:\n  " + "\n  ".join(sm_stale))
            failed = True
        if sm_untied:
            print(f"{len(sm_untied)} lines in {STRING_MODEL_FILE.relative_to(ROOT)} have no issue=#N:\n  " + "\n  ".join(sm_untied))
            failed = True
        pv_stale = sorted(i for i in private_list if i in ran and not any(r["id"] == i and private_refusal(r) for r in results))
        pv_untied = sorted(i for i, issue in private_list.items() if not re.fullmatch(r"#\d+", issue))
        if pv_stale:
            print(f"{len(pv_stale)} tests in {PRIVATE_FILE.relative_to(ROOT)} are no longer refused for a CPython-private helper "
                  "in both builds; remove them:\n  " + "\n  ".join(pv_stale))
            failed = True
        if pv_untied:
            print(f"{len(pv_untied)} lines in {PRIVATE_FILE.relative_to(ROOT)} have no issue=#N:\n  " + "\n  ".join(pv_untied))
            failed = True
        if unowned:
            print(f"{len(unowned)} known-wrong lines lack a tracking issue, owner or live expiry ({BASELINE.relative_to(ROOT)}):\n  "
                  + "\n  ".join(unowned))
            failed = True
        if new:
            print(f"{len(new)} NEW wrong results or crashes (fix them, or read each and add it with --write-baseline):\n  " + "\n  ".join(new))
            failed = True
        if gone:
            print(f"{len(gone)} known-wrong entries are no longer wrong; delete their files from {BASELINE.relative_to(ROOT)}/:\n  "
                  + "\n  ".join(f"{g}  ({entry_name(g)})" for g in gone))
            failed = True
        if not cfg.filter and cfg.files == ",".join(SCOPE):
            for m, n in matched.items():
                if m in floor and n < floor[m]:
                    print(f"{m} build: {n} tests match, below the floor of {floor[m]}; find the regression (or lower the floor on purpose)")
                    failed = True
                elif m in floor and n > floor[m]:
                    print(f"{m} build: {n} tests match, above the floor of {floor[m]}; main raises the floor after the merge (leave the floor file alone)")
        if failed:
            sys.exit(1)
        print(f"ratchet holds: no new wrong results ({len(bad)} known), matches {matched}")
        if cfg.raise_floor and not cfg.filter and cfg.files == ",".join(SCOPE):
            # Only main does this (.github/workflows/conformance-main.yml), after the ratchet held on the merged tree,
            # so PRs never edit the floors and never conflict on them.
            up = {m: max(n, floor.get(m, 0)) for m, n in matched.items()}
            if up != floor:
                FLOOR.write_text(json.dumps({**read_floor(), "matched": up}, indent=1) + "\n")
                print(f"floor raised to {up} in {FLOOR.relative_to(ROOT)}")
    if cfg.release_gate:
        # The release bar: an absolute coverage floor and no known-wrong result at all (release_gate in the floor file).
        gate, n, problems = read_floor().get("release_gate", {}), rep["totals"]["normal"], []
        if not gate:
            sys.exit(f"no release_gate in {FLOOR.relative_to(ROOT)}")
        if n["coverage"] < gate.get("coverage", 0):
            problems.append(f"coverage {n['coverage'] * 100:.1f}% is below the release gate of {gate['coverage'] * 100:.0f}%")
        if gate.get("wrong", 0) == 0 and bad:
            problems.append(f"{len(bad)} wrong results or crashes; the release gate allows none")
        if problems:
            print("release gate FAILS:\n  " + "\n  ".join(problems)); sys.exit(1)
        print(f"release gate holds: coverage {n['coverage'] * 100:.1f}%, no wrong results, {n['by_design']} refused by design, "
              f"{n['string_model']} need the string model, {n['cpython_private']} call CPython-private helpers")


if __name__ == "__main__":
    main()
