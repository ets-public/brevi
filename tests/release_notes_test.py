#!/usr/bin/env python3
"""Unit tests for tools/share.py's pct() and tools/release_notes.py's figure checks. Needs no network and no
compiler: python3 tests/release_notes_test.py"""
import json, os, shutil, sys, tempfile, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import conformance
import release_notes as rn
from share import pct


class Pct(unittest.TestCase):
    def test_rounds_once_from_the_counts(self):
        self.assertEqual(pct(945, 1072), "88.2%")  # 88.153%: rounding to 0.8815 first gave 88.1%

    def test_rounds_half_up(self):
        self.assertEqual(pct(1, 16), "6.3%")  # 6.25%: format(6.25, ".1f") gives 6.2
        self.assertEqual(pct(1, 8), "12.5%")
        self.assertEqual(pct(1, 3), "33.3%")
        self.assertEqual(pct(2, 3), "66.7%")

    def test_edges(self):
        self.assertEqual(pct(0, 5), "0.0%")
        self.assertEqual(pct(5, 5), "100.0%")
        self.assertEqual(pct(0, 0), "0.0%")
        self.assertEqual(pct("27", "1043"), "2.6%")

    def test_conformance_and_release_notes_share_it(self):
        self.assertIs(conformance.pct, pct)
        self.assertIs(rn.pct, pct)


def report(slice_dir=""):
    base = ROOT / "bench" / "conformance" / slice_dir
    js = (base / "report.json").read_text()
    return conformance.markdown(json.loads(js), []), js


class CheckFigures(unittest.TestCase):
    def test_a_report_printed_with_pct_holds(self):
        for d in ("", "stdlib"):
            md, js = report(d)
            self.assertEqual(rn.check_figures(md, js, d), [])

    def test_refuses_a_twice_rounded_share(self):
        md, js = report()
        n = json.loads(js)["totals"]["normal"]
        right = pct(n["matched_asserting"], n["scored"])
        wrong = f"{float(right[:-1]) - 0.1:.1f}%"
        bad = md.replace(f"**Assertion-bearing coverage {right}**", f"**Assertion-bearing coverage {wrong}**")
        self.assertNotEqual(bad, md)
        problems = rn.check_figures(bad, js, "report.md")
        self.assertEqual(len(problems), 1)
        self.assertIn(f"prints {wrong}", problems[0])

    def test_refuses_counts_that_are_not_report_json(self):
        md, js = report()
        j = json.loads(js)
        j["totals"]["normal"]["matched_typed"] += 1
        self.assertTrue(any("typed says" in p for p in rn.check_figures(md, json.dumps(j), "report.md")))

    def test_refuses_a_wrong_dynamic_share(self):
        md, js = report()
        j = json.loads(js)
        j["totals"]["dyn"]["matched"] -= 50
        self.assertTrue(any("forced dynamic" in p for p in rn.check_figures(md, json.dumps(j), "report.md")))

    def test_refuses_a_missing_figure(self):
        md, js = report()
        bad = "\n".join(l for l in md.splitlines() if "Typed-path share" not in l)
        self.assertTrue(any("no typed figure" in p for p in rn.check_figures(bad, js, "report.md")))


class CheckCommand(unittest.TestCase):
    """--check itself exits non-zero on a report whose share is not pct() of its counts."""

    def run_check(self, mutate):
        with tempfile.TemporaryDirectory() as t:
            for d in ("", "stdlib"):
                dst = Path(t) / "bench" / "conformance" / d
                dst.mkdir(parents=True, exist_ok=True)
                md, js = report(d)
                (dst / "report.md").write_text(mutate(md) if d == "" else md)
                (dst / "report.json").write_text(js)
            old = rn.ROOT, rn.check_reports, sys.argv
            rn.ROOT, rn.check_reports, sys.argv = t, lambda tag: None, ["release_notes.py", "--check", "v0.0.0"]
            try:
                rn.main()
            finally:
                rn.ROOT, rn.check_reports, sys.argv = old

    def test_passes_a_sound_report(self):
        self.run_check(lambda md: md)

    def test_refuses_a_wrong_share(self):
        md, _ = report()
        right = md.split("**Coverage ")[1].split("**")[0]
        with self.assertRaises(SystemExit) as e:
            self.run_check(lambda md: md.replace(f"**Coverage {right}**", "**Coverage 99.9%**"))
        self.assertIn("do not hold", str(e.exception.code))


if __name__ == "__main__":
    unittest.main()
