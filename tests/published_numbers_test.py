"""tools/published_numbers.py: the speed medians, and the scan that refuses a headline figure quoted without its scope."""
import os, subprocess, sys, tempfile, unittest
from decimal import Decimal
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import published_numbers as pn  # noqa: E402

C = {"cov": "97.3%", "dyn_cov": "97.3%", "n": {"by_design": 83}}
MED = {"typed": {"median": "9.3x"}, "release": {"median": "9.8x"}, "dynamic": {"median": "0.85x"}}
SCOPE = "of the scored suite, with 83 string-as-code tests excluded"


class Speed(unittest.TestCase):
    def test_median_of_an_even_count(self):
        self.assertEqual(pn.median([1.6, 2.0, 6.6, 12.0, 30, 37.9]), Decimal("9.3"))

    def test_times_rounds_half_up_once(self):
        self.assertEqual(pn.times(Decimal("9.75")), "9.8x")
        self.assertEqual(pn.times(Decimal("0.85")), "0.85x")
        self.assertEqual(pn.times(Decimal("0.845")), "0.85x")

    def test_the_committed_data_gives_the_published_medians(self):
        _, med = pn.speed()
        self.assertEqual((med["typed"]["median"], med["release"]["median"], med["dynamic"]["median"]),
                         ("9.3x", "9.8x", "0.85x"))
        self.assertEqual(med["dynamic"]["slower"], 7)


class Scan(unittest.TestCase):
    def test_refuses_a_bare_headline(self):
        self.assertTrue(pn.scan("x.md", "Brevi matches 97.3% of tests.", C, MED))
        self.assertTrue(pn.scan("x.md", "**97.3 %** match", C, MED))

    def test_passes_a_scoped_headline(self):
        self.assertEqual(pn.scan("x.md", f"Brevi matches 97.3% {SCOPE}.", C, MED), [])

    def test_ignores_other_numbers_and_css_widths(self):
        self.assertEqual(pn.scan("x.md", "97.3 / 454.7 ms; 197.3%; 97.31%", C, MED), [])
        self.assertEqual(pn.scan("x.html", '<i style="width: 97.3%"></i>', C, MED), [])

    def test_refuses_a_typed_median_alone(self):
        self.assertTrue(pn.scan("x.md", "median 9.3x faster", C, MED))
        self.assertTrue(pn.scan("x.md", "`--release` median 9.8x", C, MED))
        self.assertEqual(pn.scan("x.md", "median 19.3x", C, MED), [])

    def test_passes_a_typed_median_with_the_dynamic_one_and_its_scope(self):
        self.assertEqual(pn.scan("x.md", "typed median 9.3x, dynamic median 0.85x; only a small share of programs take the typed build", C, MED), [])
        self.assertEqual(pn.scan("x.md", "typed median 9.3x, dynamic median 0.85x, typed share 2.6%", dict(C, typed="2.6%"), MED), [])

    def test_refuses_a_typed_median_without_saying_few_programs_take_it(self):
        self.assertTrue(pn.scan("x.md", "typed median 9.3x, dynamic median 0.85x", C, MED))
        self.assertEqual(pn.scan("x.md", "| **Median** | **9.3x** | **0.85x** |", C, MED), [])
        self.assertEqual(pn.scan(pn.OUT, "typed median 9.3x, dynamic median 0.85x", C, MED), [])

    def test_refuses_a_rounded_share(self):
        self.assertTrue(pn.scan("x.html", "it matches <strong>97% of the tests it scores</strong>", C, MED))
        self.assertEqual(pn.scan("x.md", f"97% {SCOPE}; 197%; 97.5%; 970%", C, MED), [])


class ShallowTag(unittest.TestCase):
    """A tag fetched with --depth=1 is a shallow root, so the measured commit looked unrelated to it (PC ci_local)."""

    def run_git(self, cwd, *a):
        env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t",
                   GIT_COMMITTER_EMAIL="t@t")
        return subprocess.run(["git", "-C", cwd, *a], check=True, capture_output=True, text=True, env=env).stdout.strip()

    def commit(self, repo, path, text, msg):
        full = os.path.join(repo, path)
        os.makedirs(os.path.dirname(full), exist_ok=True)
        with open(full, "w") as f:
            f.write(text)
        self.run_git(repo, "add", "-A")
        self.run_git(repo, "commit", "-q", "-m", msg)
        return self.run_git(repo, "rev-parse", "HEAD")

    def test_deepens_a_shallow_tag_to_see_the_measured_commit(self):
        with tempfile.TemporaryDirectory() as tmp:
            origin, clone = os.path.join(tmp, "origin"), os.path.join(tmp, "clone")
            self.run_git(tmp, "init", "-q", "-b", "main", origin)
            measured = self.commit(origin, "src/a.rs", "fn a() {}\n", "release")
            head = f"Brevi 0.0.1 (commit {measured[:12]}) against CPython 3.13\n"
            self.commit(origin, "bench/conformance/report.md", head, "core report")
            tagged = self.commit(origin, "bench/conformance/stdlib/report.md", head, "stdlib report")
            self.run_git(origin, "tag", "v0.0.1")
            self.commit(origin, "src/b.rs", "fn b() {}\n", "later")
            self.run_git(tmp, "clone", "-q", "--depth=1", "--no-tags", "file://" + origin, clone)
            self.run_git(clone, "fetch", "-q", "--depth=1", "origin", "refs/tags/v0.0.1:refs/tags/v0.0.1")
            self.run_git(clone, "fetch", "-q", "--depth=1", "origin", measured)
            old = pn.ROOT
            pn.ROOT = clone
            try:
                got, tag_commit, changed = pn.provenance("v0.0.1")
            finally:
                pn.ROOT = old
            self.assertEqual((got, tag_commit), (measured, tagged))
            self.assertEqual(sorted(changed), ["bench/conformance/report.md", "bench/conformance/stdlib/report.md"])


if __name__ == "__main__":
    unittest.main()
