"""Latency fixture for bench/latency.py: a 3-file, ~200-line program in the style agents write.
bench/latency.py rewrites the EDIT line before each timed edit so every build sees new code."""
from stats import mean, median, variance, primes_upto, fib, gcd, histogram, lcg
from textutil import word_counts, top_words, is_palindrome, caesar, wrap, run_length

EDIT = 0

TEXT = [
    "The quick brown fox jumps over the lazy dog.",
    "A man, a plan, a canal: Panama!",
    "Never odd or even; the dog sleeps while the fox runs.",
    "Brevi compiles the plain Python an agent writes.",
]


def inventory() -> dict[str, list[int]]:
    stock: dict[str, list[int]] = {}
    for i, name in enumerate(["apple", "pear", "plum", "fig", "kiwi"]):
        stock[name] = [(i * 7 + k * 3) % 11 for k in range(6)]
    return stock


def report_inventory(stock: dict[str, list[int]]) -> list[str]:
    lines = []
    for name in sorted(stock):
        qty = stock[name]
        total = sum(qty)
        low = min(qty)
        flag = "LOW" if low < 2 else "ok"
        lines.append(f"{name:<6} total={total:3d} min={low} {flag}")
    return lines


def grid_paths(w: int, h: int) -> int:
    row = [1] * (w + 1)
    for _ in range(h):
        for x in range(1, w + 1):
            row[x] += row[x - 1]
    return row[w]


def main() -> None:
    print("edit", EDIT)
    counts = word_counts(TEXT)
    for w, c in top_words(counts, 3):
        print(w, c)
    print([is_palindrome(t) for t in TEXT])
    print(caesar("Hello, World", 3 + EDIT % 2))
    for line in wrap(" ".join(TEXT), 30):
        print(line)
    print(run_length("aaabccddddd"))
    xs = [float(x) for x in lcg(42 + EDIT, 50)]
    print(round(mean(xs), 3), median(xs), round(variance(xs), 3))
    print(len(primes_upto(1000)), fib(30), gcd(462, 1071))
    print(histogram(lcg(7, 100), 5, 0, 1000))
    for line in report_inventory(inventory()):
        print(line)
    print(grid_paths(8, 8))


main()
