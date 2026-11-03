Turn comma-separated rows from stdin into a Markdown-style text table. Lines are numbered from 1. The first non-blank line is the header; every later non-blank line is a data row. Blank lines are ignored. Cells are split on `,` (there is no quoting) and have spaces stripped from both ends.

- A data row with fewer cells than the header is padded with empty cells. A data row with more cells is left out of the table, and after the table the program prints `skipped line N: K cells` for each one, in input order.
- A column is numeric when it has at least one non-empty cell and every non-empty cell matches `-?[0-9]+(\.[0-9]+)?`. In a numeric column every non-empty cell is shown with as many decimal places as the cell with the most decimals in that column (pad with zeros: `2` becomes `2.00` if another cell is `1.25`).
- If at least one column is numeric, the table ends with a totals row: each numeric column shows the sum of its cells, with the column's decimal places; the first column shows `total` unless it is numeric; other text columns are empty.

Print the header row, then a separator row, then the data rows in input order, then the totals row if any. Each row is `| ` + the cells joined by ` | ` + ` |`. Each column is as wide as its widest entry among the header, the shown cells and the totals row (including the word `total`). Text cells are padded on the right, numeric cells on the left; header names are padded on the right in every column. The separator has, for each column, `-` repeated (width + 2) times for a text column, or `-` repeated (width + 1) times followed by `:` for a numeric column, joined by `|`, with `|` at both ends.

Finish with `R rows, C columns` (R = data rows shown, C = header cells). If the input has no header, print only `0 rows, 0 columns`.
