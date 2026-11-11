#!/usr/bin/env python3
"""Unit tests for the extractor and classifier in tools/conformance.py. Needs no network and no compiler:
  python3 tests/conformance_test.py"""
import subprocess, sys, tempfile, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import conformance as c

HEAD = "import unittest\nclass T(unittest.TestCase):\n"


def universe(body, head=HEAD):
    return c.Universe({"test.test_x": (head + body).encode()})


def cut(body, head=HEAD, cls="T", method="test_a"):
    return universe(body, head).extract("test_x", cls, method)


def run(script):
    with tempfile.TemporaryDirectory() as t:
        f = Path(t) / "case.py"
        f.write_text(script)
        return subprocess.run([sys.executable, str(f)], capture_output=True, text=True)


class Assertions(unittest.TestCase):
    def rewritten(self, line):
        return cut(f"    def test_a(self):\n        {line}\n").split("def test_a():\n")[1].split("\n\ntest_a()")[0].strip()

    def test_equal_prints_repr(self):
        self.assertEqual(self.rewritten("self.assertEqual(f(x), 1)"), "print(repr(f(x)))")

    def test_equal_keeps_a_second_expression(self):
        self.assertEqual(self.rewritten("self.assertEqual(a, g())"), "print(repr(a))\n    print(repr(g()))")

    def test_equal_keeps_literal_spelling(self):
        self.assertEqual(self.rewritten("self.assertEqual(0xff, 0o377)"), "print(repr(0xff))\n    print(repr(0o377))")

    def test_msg_is_dropped(self):
        self.assertEqual(self.rewritten("self.assertEqual(a, 1, 'why')"), "print(repr(a))")
        self.assertEqual(self.rewritten("self.assertTrue(a, msg='why')"), "print(repr(bool(a)))")

    def test_truth_and_comparisons(self):
        self.assertEqual(self.rewritten("self.assertFalse(a)"), "print(repr(bool(a)))")
        self.assertEqual(self.rewritten("self.assertIs(a, b)"), "print((a) is (b))")
        self.assertEqual(self.rewritten("self.assertNotIn(a, b)"), "print((a) not in (b))")
        self.assertEqual(self.rewritten("self.assertIsInstance(a, int)"), "print(isinstance(a, int))")

    def test_raises_with_a_callable(self):
        out = run(cut("    def test_a(self):\n        self.assertRaises(ValueError, int, 'x')\n"))
        self.assertEqual(out.stdout, "raises ValueError\n")

    def test_raises_as_a_with_block_and_its_exception(self):
        s = cut("    def test_a(self):\n        with self.assertRaises(KeyError) as cm:\n            {}['k']\n"
                "        self.assertEqual(cm.exception.args, ('k',))\n")
        self.assertEqual(run(s).stdout, "raises KeyError\n('k',)\n('k',)\n")

    def test_raises_regex_prints_the_message(self):
        s = cut("    def test_a(self):\n        with self.assertRaisesRegex(ValueError, 'bad'):\n            raise ValueError('bad one')\n")
        self.assertEqual(run(s).stdout, "raises ValueError bad one\n")

    def test_subtest_keeps_the_body(self):
        s = cut("    def test_a(self):\n        for i in (1, 2):\n            with self.subTest(i=i):\n                self.assertEqual(i, 0)\n")
        self.assertEqual(run(s).stdout, "1\n2\n")

    def test_unknown_assertion_is_not_extractable(self):
        with self.assertRaisesRegex(c.CantExtract, "assertLogs"):
            cut("    def test_a(self):\n        with self.assertLogs('x'):\n            pass\n")

    def test_assert_warns_records_the_warning(self):
        s = cut("    def test_a(self):\n        import warnings\n        with self.assertWarns(DeprecationWarning):\n            warnings.warn('old', DeprecationWarning)\n")
        self.assertEqual(run(s).stdout, "warns True ['DeprecationWarning']\n")

    def test_skip_condition_is_decided_by_the_reference_python(self):
        u = universe("    @unittest.skipIf(sys.platform != 'nonexistent', 'never runs')\n    def test_a(self):\n        pass\n"
                     "    @unittest.skipUnless(sys.platform != 'nonexistent', 'always runs')\n    def test_b(self):\n        pass\n",
                     "import sys\n" + HEAD)
        u.python = sys.executable
        with self.assertRaisesRegex(c.Excluded, "skipped by CPython: never runs"):
            u.extract("test_x", "T", "test_a")
        self.assertEqual(run(u.extract("test_x", "T", "test_b")).returncode, 0)
        with self.assertRaisesRegex(c.CantExtract, "cannot be decided"):
            cut("    @unittest.skipIf(open('x'), 'no')\n    def test_a(self):\n        pass\n")

    def test_assertion_inside_a_lambda_is_not_extractable(self):
        with self.assertRaisesRegex(c.CantExtract, "assertEqual"):
            cut("    def test_a(self):\n        f = lambda: self.assertEqual(1, 1)\n        f()\n")


class Inlining(unittest.TestCase):
    def test_a_fresh_module_copy_is_the_module(self):
        s = cut("    def test_a(self):\n        self.assertEqual(py_json.dumps([1]), '[1]')\n",
                head="import unittest\nfrom test.support import import_helper\n"
                     "py_json = import_helper.import_fresh_module('json', blocked=['_json'])\nclass T(unittest.TestCase):\n")
        self.assertIn("import json as py_json", s)
        self.assertNotIn("import_helper", s)
        self.assertEqual(run(s).stdout, "'[1]'\n")

    def test_helpers_and_setup_come_along(self):
        s = cut("    def setUp(self):\n        self.v = K\n    def twice(self):\n        return helper(self.v)\n"
                "    def test_a(self):\n        self.assertEqual(self.twice(), 8)\n    def test_b(self):\n        pass\n",
                head="import unittest\nK = 4\nUNUSED = 1\ndef helper(x):\n    return x * 2\nclass T(unittest.TestCase):\n")
        self.assertNotIn("UNUSED", s)
        self.assertNotIn("test_b", s)
        self.assertEqual(run(s).stdout, "8\n")

    def test_inherited_test_runs_with_its_subclass_attribute(self):
        head = "import unittest\nclass B(unittest.TestCase):\n    kind = None\n    def test_a(self):\n        self.assertEqual(self.kind(), [])\n"
        u = universe("    kind = list\n", head + "class T(B):\n")
        self.assertEqual(sorted(u.tests("test_x")), [("B", "test_a"), ("T", "test_a")])
        self.assertEqual(run(u.extract("test_x", "T", "test_a")).stdout, "[]\n[]\n")

    def test_multi_line_string_keeps_its_spaces(self):
        s = cut('    def test_a(self):\n        s = """\n        x\n"""\n        self.assertEqual(s, 1)\n')
        self.assertEqual(run(s).stdout, repr("\n        x\n") + "\n")

    def test_unsafe_import_is_not_extractable(self):
        with self.assertRaisesRegex(c.CantExtract, "imports os"):
            cut("    def test_a(self):\n        import os\n        os.getcwd()\n")

    def test_open_is_not_extractable(self):
        with self.assertRaisesRegex(c.CantExtract, "open"):
            cut("    def test_a(self):\n        open('x')\n")

    def test_cpython_only_is_excluded(self):
        with self.assertRaisesRegex(c.Excluded, "cpython_only"):
            cut("    @support.cpython_only\n    def test_a(self):\n        pass\n")


class Classify(unittest.TestCase):
    want = c.Run(0, b"1\n2\n", b"", "")

    def test_each_outcome(self):
        built = (True, "", False)
        self.assertEqual(c.classify(self.want, built, c.Run(0, b"1\n2\n", b"", ""))[0], "matched")
        self.assertEqual(c.classify(self.want, built, c.Run(0, b"1\n3\n", b"", ""))[0], "wrong_output")
        self.assertEqual(c.classify(self.want, built, c.Run(1, b"1\n", b"boom", ""))[0], "wrong_exit")
        self.assertEqual(c.classify(self.want, built, c.Run(-9, b"", b"", "timeout"))[0], "crash_or_timeout")
        self.assertEqual(c.classify(self.want, built, c.Run(-11, b"", b"", ""))[1], "signal SIGSEGV")
        panic = c.Run(1, b"", b"thread 'x' panicked at /home/me/rt.rs:1:2:\nRefCell already borrowed\nnote: run with\n", "")
        self.assertEqual(c.classify(self.want, built, panic), ("crash_or_timeout", "panic: RefCell already borrowed"))
        self.assertEqual(c.classify(self.want, (False, "no", False), None), ("refused", "no"))
        self.assertEqual(c.classify(self.want, (False, "panic", True), None)[0], "crash_or_timeout")


if __name__ == "__main__":
    unittest.main()
