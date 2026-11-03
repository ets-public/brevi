Find long Collatz chains. The chain of a positive integer n repeats `n = n / 2` (n even) or `n = 3n + 1` (n odd) until n is 1; its length counts the numbers in it, both ends included (the chain of 1 has length 1, of 6 has length 9).

Each line of stdin is a query `A B` (1 <= A <= B <= 3000000). Blank lines are skipped; any other line that is not two integers in range with A <= B prints `line L: bad query` (lines counted from 1) and is skipped.

For each query print `A..B: N has length K`, where N is the number in A..B with the longest chain (the smallest such number on a tie) and K its length. At the end print `Q queries`, counting the queries answered.
