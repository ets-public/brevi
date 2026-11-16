"""tools/published_numbers.py: the speed medians, and the scan that refuses a headline figure quoted without its scope."""
import sys, unittest
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

    def test_passes_a_typed_median_with_the_dynamic_one(self):
        self.assertEqual(pn.scan("x.md", "typed median 9.3x, dynamic median 0.85x", C, MED), [])


if __name__ == "__main__":
    unittest.main()
