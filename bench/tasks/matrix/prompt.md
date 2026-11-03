Write a calculator for matrices of exact fractions. Each line of stdin is a command; lines are numbered from 1 and blank lines are ignored. Names are a letter followed by letters or digits.

- `NAME = [ROW; ROW; ...]` defines a matrix. A ROW is numbers separated by spaces; every row must have the same number of entries, at least one. A number is an integer (`-3`), a fraction of integers (`2/3`, `-1/4`; not divided by zero) or a decimal (`0.5`, `-1.25`).
- `NAME = X + Y`, `NAME = X - Y`, `NAME = X * Y` (matrix product), `NAME = X'` (transpose), `NAME = inv X` (inverse), `NAME = X ^ K` (X multiplied by itself K times, K a whole number of 0 or more; `X ^ 0` is the identity), where X and Y are defined names. Operators and names are separated by spaces, except `'`, which follows the name directly.
- `print X` prints `X = [ROW; ROW; ...]` in the same form, with each entry as a reduced fraction (`3`, `-1/2`, `0`).
- `det X` prints `det X = D`; `rank X` prints `rank X = R`.

Errors print one line and change nothing: `line N: unknown NAME` for an undefined name (the first one in the line), `line N: size mismatch` for sizes that do not fit (+ and - need the same size, X * Y needs X's column count to equal Y's row count), `line N: not square` when inv, det or ^ gets a non-square matrix, `line N: singular` when inv gets a matrix whose determinant is 0, and `line N: bad line` for anything else (including a matrix literal with uneven rows or a bad number).
