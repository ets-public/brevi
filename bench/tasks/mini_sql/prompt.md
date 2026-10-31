Run a tiny SQL database. Read statements from stdin, one per line (ignore blank lines). Keywords are upper case and words are separated by single spaces, exactly as below. Names are letters, digits and `_`, starting with a letter. A value is a whole number (optionally negative) or a string in single quotes (`''` inside it is one quote; it may contain spaces and commas).

- `CREATE TABLE T (COL TYPE, COL TYPE, ...)` where TYPE is `INT` or `TEXT`; prints `ok`. Creating a table that exists is `error: table T exists`.
- `INSERT INTO T VALUES (V, V, ...)`: one value per column, in order, of the column's type; prints `ok`.
- `SELECT COLS FROM T [WHERE COND] [ORDER BY COL [DESC]] [LIMIT N]`: COLS is `*` (all columns in table order) or column names separated by `, `. Prints the column names joined by ` | `, then each matching row the same way (strings without quotes), then `(N rows)`. Without ORDER BY, rows come in insertion order; ORDER BY sorts ascending (INT by number, TEXT by character code), DESC descending, and equal values keep insertion order either way. LIMIT N keeps the first N rows after sorting.
- `SELECT COUNT(*) FROM T [WHERE COND]`: prints the count.
- `UPDATE T SET COL = V [WHERE COND]`: prints `updated N`.
- `DELETE FROM T [WHERE COND]`: prints `deleted N`.

COND is one or more comparisons joined by ` AND `: `COL OP V` with OP one of `=`, `!=`, `<`, `>`, `<=`, `>=`.

Errors print one line and change nothing: `error: no table T`; `error: no column C` (the first unknown column in the statement, reading left to right); `error: type mismatch` when an INSERT has the wrong number of values or a value of the wrong type, or a comparison or SET value does not match its column's type; `error: syntax` for any line that is not one of the forms above (checked before anything else).
