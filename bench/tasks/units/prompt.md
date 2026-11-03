Convert between units from facts given on stdin. Lines are numbered from 1; blank lines are ignored. Unit names are one or more ASCII letters and are case-sensitive. NUMBER below is digits with an optional `.` and more digits (`3`, `0.001`).

- `1 A = X B` says one A is X B (X is a NUMBER greater than 0, and A and B are different units). Conversions work both ways and chain through other units. If A and B can already be converted through earlier facts, the fact is ignored and the program prints `line N: already known` if it gives the same factor (exactly) or `line N: conflict` otherwise.
- `? Q A in B` converts the NUMBER Q of unit A to unit B and prints `Q A = V B`, with Q exactly as written and V formatted as Python's format spec `.6g` prints the float of the exact result. If A or B never appeared in an accepted fact print `unknown unit U` (checking A first). If both are known but there is no chain from A to B, print `cannot convert A to B`. A unit converts to itself with factor 1.
- Anything else prints `line N: bad line`.

Work out conversion factors exactly (as fractions) and turn the result into a float only to print it.
