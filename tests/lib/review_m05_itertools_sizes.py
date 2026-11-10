from itertools import islice, permutations, combinations, combinations_with_replacement, product
cases = [lambda: list(islice(range(10), -1)), lambda: list(islice(range(10), -5)), lambda: list(islice(range(10), 2**70)),
 lambda: list(islice(range(10), 1.5)), lambda: list(islice(range(10), -1, 5)), lambda: list(islice(range(10), 1, 8, 0)),
 lambda: list(islice(range(10), 1, None, 3)), lambda: list(islice(range(10), None, 4)), lambda: list(islice(range(10), "a", 4)),
 lambda: list(islice(range(10), 2, 9, 2**70)), lambda: list(islice(range(10), stop=3)), lambda: list(islice(range(10))),
 lambda: list(combinations([1,2], -1)), lambda: list(permutations([1,2], -2)), lambda: list(product([1], repeat=-1)),
 lambda: list(permutations([1,2], 2**70)), lambda: list(combinations([1,2,3], 5)), lambda: list(product([1,2], repeat=0)),
 lambda: list(combinations_with_replacement("ab", 3)), lambda: list(permutations("abc", None)), lambda: list(islice(iter([1,2,3]), True))]
for c in cases:
    try: print(c())
    except Exception as e: print(type(e).__name__, e)
