Solve sudoku puzzles. Stdin holds one or more puzzles separated by blank lines (several blank lines count as one; blank lines at the start or end are ignored). A puzzle is 9 lines of 9 characters: a digit `1` to `9` for a given cell, or `.` for an empty one. Puzzles are numbered from 1.

For each puzzle print:
- `puzzle K: bad input` if it is not 9 lines of 9 such characters.
- `puzzle K: invalid` if two givens with the same digit share a row, a column or a 3x3 box.
- `puzzle K: no solution` if the empty cells cannot be filled so that every row, column and 3x3 box holds the digits 1 to 9.
- `puzzle K: multiple solutions` if there is more than one way.
- otherwise `puzzle K: solved`, then the 9 lines of the solved grid.
