Read two CSV tables and answer queries about them. Stdin has three parts separated by lines that are exactly `---`: table `a`, table `b`, then queries. Each table starts with a header row. Rows are read as Python's `csv` module reads them (fields may be quoted with `"`, contain commas, and use `""` for a quote); no field contains a line break. Lines are numbered from 1 over the whole input. Blank lines are ignored everywhere.

When a table row has a different number of fields from its header, the row is dropped; before answering any query, print `line N: wrong field count` for each dropped row in input order.

Queries, one per line:
- `join KEY`: an inner join on column KEY, which both tables must have. The result has a's columns, then b's columns except KEY; a b column whose name is already used gets `_b` added. For each a row in order, one result row per b row with the same KEY value, in b's order.
- `left KEY`: the same, but an a row with no match gives one row with empty b fields.
- `count T COL`: for table T (`a` or `b`), the distinct values of COL with how many rows have each, as rows `value,count` under the header `COL,count`, by count (largest first) then value.
- `sum T COL BY`: for table T, adds up COL for each distinct value of BY, as rows `value,total` under the header `BY,COL`, sorted by value. Every COL value in T must be a decimal number (`-?[0-9]+(\.[0-9]+)?`); totals have as many decimal places as the most precise value of COL in the table.

Print each result as CSV the way Python's `csv.writer` writes it with `lineterminator="\n"` (fields quoted only when needed), header row first, then a line `(R rows)`.

Errors print one line instead of a result: `line N: unknown table T`, `line N: unknown column C` (the first column named in the query that the table lacks; for joins, check a first, then b), `line N: COL is not numeric`, or `line N: bad query` for anything else.
