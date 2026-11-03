Solve 0/1 knapsack problems. Each line of stdin is one problem: `N CAP SEED` (1 <= N <= 1000, 0 <= CAP <= 300000). Lines that are blank are skipped; any other line that is not three non-negative integers in those ranges prints `line L: bad line` (lines counted from 1) and is skipped.

Random numbers come from this generator: start with `x = SEED`; each next number is `x = (x * 1103515245 + 12345) % 2147483648`, and the number used is the new `x`.

Item i (from 1 to N) takes two random numbers in turn: its weight is `1 + first % 1000` and its value is `1 + second % 1000`.

Choose a set of items, each used at most once, whose total weight is at most CAP and whose total value is as large as possible. Print `line L: best V weight W`, where V is that largest total value and W is the smallest total weight of any set reaching V.
