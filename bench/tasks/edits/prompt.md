Compute edit distances between generated DNA strings. Each line of stdin is one pair: `LEN_A LEN_B SEED` (0 <= LEN_A, LEN_B <= 10000). Blank lines are skipped; any other line that is not three non-negative integers with lengths in range prints `line L: bad line` (lines counted from 1) and is skipped.

Random numbers come from this generator: start with `x = SEED`; each next number is `x = (x * 1103515245 + 12345) % 2147483648`, and the number used is the new `x`.

String A takes LEN_A random numbers, then string B takes LEN_B; each number gives the letter `"acgt"[number % 4]`.

Print `line L: distance D same S`, where D is the Levenshtein distance between A and B (the fewest single-letter insertions, deletions and substitutions that turn A into B) and S is the length of their longest common subsequence.
